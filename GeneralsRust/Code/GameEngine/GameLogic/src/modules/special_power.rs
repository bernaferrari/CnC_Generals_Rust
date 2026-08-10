// Special power, stealth, and countermeasure interfaces
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Special Power module interface (matching C++ SpecialPowerModuleInterface)
pub trait SpecialPowerModuleInterface: Send + Sync {
    /// Activate the special power
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Check if the special power can be activated
    fn can_activate(&self) -> bool;
    /// Get the special power type
    fn get_power_type(&self) -> u32;
    /// Restart power recharge timer
    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Get the frame when this power will be ready
    fn get_ready_frame(&self) -> u32;
    /// Check if the power is ready to fire
    fn is_ready(&self) -> bool;
    /// Get the special power template associated with this module
    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>>;

    /// Get the special power template as a concrete type when possible.
    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        None
    }

    /// Whether this module corresponds to the supplied template.
    fn is_module_for_power(&self, _special_power_template: &SpecialPowerTemplate) -> bool {
        false
    }

    /// Force a ready frame (used by script-fired special powers).
    fn set_ready_frame(&mut self, _frame: u32) {}

    // New methods from special_power_module.rs for full functionality
    fn get_power_name(&self) -> String;
    fn get_percent_ready(&self) -> f32;
    fn pause_countdown(&mut self, pause: bool);
    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>);
    fn on_special_power_creation(&mut self) {
        let _ = self.get_ready_frame();
    }

    /// Execute special power with no target (default: no-op).
    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        let _ = command_options;
    }

    /// Execute special power at object (default: no-op).
    fn do_special_power_at_object(
        &mut self,
        _object_id: ObjectID,
        _command_options: SpecialPowerCommandOptions,
    ) {
    }

    /// Execute special power at location (default: no-op).
    fn do_special_power_at_location(
        &mut self,
        _location: &Coord3D,
        _angle: f32,
        _command_options: SpecialPowerCommandOptions,
    ) {
    }

    /// Execute special power using waypoints (default: no-op).
    fn do_special_power_using_waypoints(
        &mut self,
        _waypoint: &Waypoint,
        _command_options: SpecialPowerCommandOptions,
    ) {
    }
}

/// Extension trait for Arc<Mutex<dyn SpecialPowerModuleInterface>> to provide convenient methods
pub trait SpecialPowerModuleInterfaceExt {
    fn pause_countdown(&self, pause: bool);
    fn is_ready(&self) -> bool;
    fn get_percent_ready(&self) -> f32;
    fn get_power_name(&self) -> String;
}

impl SpecialPowerModuleInterfaceExt for Arc<Mutex<dyn SpecialPowerModuleInterface>> {
    fn pause_countdown(&self, pause: bool) {
        if let Ok(mut guard) = self.try_lock() {
            SpecialPowerModuleInterface::pause_countdown(&mut *guard, pause);
        }
    }

    fn is_ready(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_ready()
        } else {
            false
        }
    }

    fn get_percent_ready(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_percent_ready()
        } else {
            0.0
        }
    }

    fn get_power_name(&self) -> String {
        if let Ok(guard) = self.try_lock() {
            guard.get_power_name()
        } else {
            String::from("Unknown")
        }
    }
}

/// Extension trait for Arc<Mutex<dyn SpawnBehaviorInterface>> to provide convenient methods
pub trait SpawnBehaviorInterfaceExt {
    fn order_slaves_to_clear_disabled(
        &self,
        disabled_type: DisabledType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_spawn_count(&self) -> u32;
    fn get_spawn_object(&self, index: u32) -> Option<ObjectID>;
}

impl SpawnBehaviorInterfaceExt for Arc<Mutex<dyn SpawnBehaviorInterface>> {
    fn order_slaves_to_clear_disabled(
        &self,
        disabled_type: DisabledType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_slaves_to_clear_disabled(disabled_type)
        } else {
            Ok(())
        }
    }

    fn get_spawn_count(&self) -> u32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_spawn_count()
        } else {
            0
        }
    }

    fn get_spawn_object(&self, index: u32) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_spawn_object(index)
        } else {
            None
        }
    }
}

/// Special Power Update interface
pub trait SpecialPowerUpdateInterface: Send + Sync {
    /// Does this special power update pass science test
    fn does_special_power_update_pass_science_test(&self) -> bool {
        self.get_extra_required_science() == SCIENCE_INVALID
    }
    /// Get extra required science
    fn get_extra_required_science(&self) -> ScienceType {
        SCIENCE_INVALID
    }
    /// Initiate intent to use the special power
    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        target_obj: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> bool;
    /// Is this a special ability (vs superweapon)
    fn is_special_ability(&self) -> bool;
    /// Is this a special power
    fn is_special_power(&self) -> bool;
    /// Is power active
    fn is_active(&self) -> bool;
    /// Get command option
    fn get_command_option(&self) -> SpecialPowerCommandOption;
    /// Does power have overridable destination active now
    fn does_special_power_have_overridable_destination_active(&self) -> bool;
    /// Does power have overridable destination even if not active
    fn does_special_power_have_overridable_destination(&self) -> bool;
    /// Set overridable destination
    fn set_special_power_overridable_destination(&mut self, location: &Coord3D);
    /// Is power currently in use
    fn is_power_currently_in_use(
        &self,
        _command: Option<&crate::command_button::CommandButton>,
    ) -> bool;
    /// Update special power (added to match implementation)
    fn update_special_power(
        &mut self,
        _frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Is power ready (added to match implementation)
    fn is_power_ready(&self) -> bool {
        false
    }
}

/// Special Ability Update interface
pub trait SpecialAbilityUpdate: Send + Sync {
    /// Update the special ability
    fn update_ability(
        &mut self,
        frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Check if ability is active
    fn is_ability_active(&self) -> bool;
}

/// Countermeasures Behavior interface
pub trait CountermeasuresBehaviorInterface: Send + Sync {
    /// Deploy countermeasures
    fn deploy_countermeasures(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Check if countermeasures are available
    fn has_countermeasures(&self) -> bool {
        false
    }
    /// Report a missile for countermeasure processing
    fn report_missile_for_countermeasures(
        &mut self,
        _missile_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Calculate which countermeasure to divert to
    fn calculate_countermeasure_to_divert_to(
        &self,
        _victim_id: ObjectID,
    ) -> Result<ObjectID, Box<dyn std::error::Error + Send + Sync>> {
        Ok(INVALID_ID)
    }
    /// Reload countermeasures
    fn reload_countermeasures(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Check if countermeasures are active
    fn is_active(&self) -> bool {
        self.has_countermeasures()
    }
}

/// Cleanup Hazard Update interface (matching C++ CleanupHazardUpdate)
pub trait CleanupHazardUpdateInterface: Send + Sync {
    /// Set cleanup area parameters
    fn set_cleanup_area_parameters(&mut self, pos: &Coord3D, range: Real);
}

/// Stealth Update interface
pub trait StealthUpdate: Send + Sync + std::fmt::Debug {
    /// Update stealth state
    fn update_stealth(
        &mut self,
        frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Check if object is currently stealthed
    fn is_stealthed(&self) -> bool;
    /// Begin stealth mode
    fn begin_stealth(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// End stealth mode
    fn end_stealth(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Check if stealth is allowed for this object
    fn allowed_to_stealth(&self, _object: &crate::object::Object) -> bool {
        true // Default implementation allows stealth
    }
    /// Mark object as detected (breaks stealth)
    fn mark_as_detected(&mut self) {
        // Default implementation - subclasses should override
        let _ = self.end_stealth();
    }
}
