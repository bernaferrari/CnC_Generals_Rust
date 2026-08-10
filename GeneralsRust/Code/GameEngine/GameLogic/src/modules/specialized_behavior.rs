// SlowDeath/Spawn/Horde/Exit and module constants
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Slow death behavior interface
pub trait SlowDeathBehaviorInterface: Send + Sync {
    /// Check if slow death is active
    fn is_slow_death_active(&self) -> bool;
    /// Get slow death phase
    /// Begin slow death process
    fn begin_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Get probability modifier for slow death
    fn get_probability_modifier(&self, damage_info: &DamageInfo) -> Int;
    /// Check if die is applicable
    fn is_die_applicable(&self, damage_info: &DamageInfo) -> bool;
    fn get_slow_death_phase(&self) -> u32;
}

/// Spawn behavior interface
pub trait SpawnBehaviorInterface: Send + Sync {
    /// Get number of spawned objects
    fn get_spawn_count(&self) -> u32;
    /// Get spawn object by index
    fn get_spawn_object(&self, index: u32) -> Option<ObjectID>;
    /// Order slaves to clear the specified disabled type
    fn order_slaves_to_clear_disabled(
        &mut self,
        _disabled_type: DisabledType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// Horde update interface (API used by group behavior and modules)
pub trait HordeUpdateInterface: Send + Sync + crate::common::AsAny {
    fn is_true_horde_member(&self) -> bool {
        false
    }

    fn is_in_horde(&self) -> bool {
        false
    }

    fn is_allowed_nationalism(&self) -> bool {
        true
    }
}

/// Power plant update interface (overcharge behavior uses this)
pub trait PowerPlantUpdateInterface: Send + Sync {
    /// Extend or retract reactor rods (matches C++ PowerPlantUpdateInterface::extendRods).
    fn extend_rods(&mut self, extend: Bool) {
        let _ = extend;
    }
}

/// Railed transport dock update interface
pub trait RailedTransportDockUpdateInterface: Send + Sync {
    fn is_loading_or_unloading(&self) -> bool;
    fn unload_all(&mut self);
    fn unload_single_object(&mut self, obj_id: ObjectID);
}

pub trait POWTruckAIUpdateInterface: Send + Sync {
    fn set_task(
        &mut self,
        task: crate::pow_truck_ai_update::POWTruckTask,
        task_object: Option<ObjectID>,
    );
    fn get_current_task(&self) -> crate::pow_truck_ai_update::POWTruckTask;
    fn load_prisoner(&mut self, prisoner: ObjectID);
    fn unload_prisoners_to_prison(&mut self, prison_id: ObjectID);
}

pub trait HackInternetAIUpdateInterface: Send + Sync {
    fn is_hacking(&self) -> bool;
    fn is_hacking_packing_or_unpacking(&self) -> bool;
}

pub trait AssaultTransportAIUpdateInterface: Send + Sync {
    fn begin_assault(&mut self, designated_target: Option<ObjectID>);
}

pub trait DeliverPayloadAIUpdateInterface: Send + Sync {
    fn deliver_payload(
        &mut self,
        move_to_pos: &Coord3D,
        target_pos: &Coord3D,
        data: &DeliverPayloadData,
    );
    fn deliver_payload_via_module_data(&mut self, move_to_pos: &Coord3D);
    fn is_delivering_payload(&self) -> Bool;
    fn is_allowed_to_respond_to_ai_commands(&self) -> Bool;
}

/// Module interface base trait
pub trait ModuleInterface {
    fn get_interface_type(&self) -> u32;
}

/// Slaved update interface
pub trait SlavedUpdateInterface {
    fn slaved_update(&mut self, object_id: ObjectID, delta_time: Real);

    /// Current slaver/master object, if any.
    fn slaver_id(&self) -> Option<ObjectID> {
        None
    }

    /// Called when this object becomes enslaved to a master
    fn on_enslave(
        &mut self,
        _master_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }

    /// Returns true if this slave is self-tasking (managing its own AI)
    fn is_self_tasking(&self) -> bool {
        false // Default implementation returns false
    }

    /// Called when the slaver/master dies
    fn on_slaver_die(
        &mut self,
        _damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }

    /// Called when the slaver/master takes damage
    fn on_slaver_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }
}

/// Disabled type enumeration (re-exported from common::types)
pub use crate::common::types::DisabledType;

/// Damage module interface
pub trait DamageModuleInterface: Send + Sync {
    fn receive_damage(&mut self, object_id: ObjectID, damage: &DamageInfo) -> Real;
    /// Called when damage is received
    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }
    /// Called when healing is received
    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }
    /// Called when body damage state changes
    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation does nothing
    }
}

/// Exit interface
pub trait ExitInterface {
    fn can_exit(&self, object_id: ObjectID) -> bool;
    fn exit(&mut self, object_id: ObjectID) -> bool;
    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(None)
    }

    // Additional methods needed by SpawnBehavior
    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        _spawn: Option<&crate::object::Object>,
    ) -> ExitDoorType {
        DOOR_NONE_AVAILABLE
    }

    fn unreserve_door_for_exit(&mut self, _door: ExitDoorType) {
        // Default implementation does nothing
    }

    fn exit_object_via_door(
        &mut self,
        _obj_id: ObjectID,
        _door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Special tunnel-network style exit that preserves the passenger's current AI state.
    fn exit_object_in_a_hurry(
        &mut self,
        _obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        _obj_id: ObjectID,
        _host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// Exit door type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDoorType {
    None,
    NoneAvailable,
    Primary,
    Secondary,
    Emergency,
    Door1,
    Door2,
    Door3,
    Door4,
}

// Module constants - using enum variants
pub const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
pub const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
pub const UPDATE_SLEEP_INVALID: UpdateSleepTime = UpdateSleepTime::None;
pub const UPDATE_SLEEP: UpdateSleepTime = UpdateSleepTime::Frames(1);

/// Module interface mask constants
pub const MODULEINTERFACE_UPDATE: u32 = 1 << 0;
pub const MODULEINTERFACE_DIE: u32 = 1 << 1;
pub const MODULEINTERFACE_DAMAGE: u32 = 1 << 2;
pub const MODULEINTERFACE_CREATE: u32 = 1 << 3;
pub const MODULEINTERFACE_DESTROY: u32 = 1 << 4;

// Disabled types
pub const DISABLED_HELD: DisabledType = DisabledType::Held;

// Door constants
pub const DOOR_NONE_AVAILABLE: ExitDoorType = ExitDoorType::None;

/// Extension trait for Arc<Mutex<dyn ExitInterface>> to provide convenient methods
pub trait ExitInterfaceExt {
    fn unreserve_door_for_exit(&self, door: ExitDoorType);
    fn reserve_door_for_exit(&self, spawner: Option<&str>, spawn: Option<ObjectID>)
        -> ExitDoorType;
}

impl ExitInterfaceExt for Arc<Mutex<dyn ExitInterface>> {
    fn unreserve_door_for_exit(&self, door: ExitDoorType) {
        if let Ok(mut guard) = self.try_lock() {
            guard.unreserve_door_for_exit(door);
        }
    }

    fn reserve_door_for_exit(
        &self,
        spawner: Option<&str>,
        spawn: Option<ObjectID>,
    ) -> ExitDoorType {
        if let Ok(mut guard) = self.try_lock() {
            let _ = spawner;
            let _ = spawn;
            guard.reserve_door_for_exit(None, None)
        } else {
            DOOR_NONE_AVAILABLE
        }
    }
}
