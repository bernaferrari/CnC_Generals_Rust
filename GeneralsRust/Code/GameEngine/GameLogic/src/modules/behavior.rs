// BehaviorModule and related control interfaces
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

// Module interface traits matching C++ interfaces

/// Base interface for all behavior modules (matching C++ BehaviorModuleInterface)
pub trait BehaviorModuleInterface: Send + Sync + AsAny + Any + 'static {
    /// Update the behavior module
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Get the module name/type
    fn get_module_name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    /// Get the module name key (used for module lookups by name)
    fn get_module_name_key(&self) -> NameKeyType {
        0
    }
    /// Optional typed query used by callers that need disguise-owner context.
    fn get_disguised_player_index(&self) -> Option<Int> {
        None
    }
    /// Enable/disable slow-death or similar behavior toggles (default no-op).
    fn set_sd_enabled(&mut self, enabled: bool) {
        let _ = enabled;
    }
    fn get_deletion_lifetime_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::DeletionLifetimeInterface> {
        None
    }
    fn get_bone_fx_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::BoneFxControlInterface> {
        None
    }
    fn get_prone_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::ProneControlInterface> {
        None
    }
    fn get_sticky_bomb_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::StickyBombControlInterface> {
        None
    }
    fn get_hijacker_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::HijackerControlInterface> {
        None
    }
    fn get_spy_vision_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::SpyVisionControlInterface> {
        None
    }
    fn get_topple_control_interface(&mut self) -> Option<&mut dyn ToppleControlInterface> {
        None
    }
    /// Get interface mask (indicating which interfaces this module supports)
    fn get_interface_mask() -> u32
    where
        Self: Sized,
    {
        0
    }
    /// Called when the object is created
    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = self.get_module_name_key();
        Ok(())
    }
    /// Called when the object dies
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Called when the object becomes disabled or re-enabled.
    fn on_disabled_edge(&mut self, now_disabled: bool) {
        let _ = now_disabled;
    }
    /// Called when the object is captured by a new owner.
    fn on_capture(
        &mut self,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        _new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
    }
    /// Core module interface hooks
    fn get_body(&mut self) -> Option<&mut dyn BodyModuleInterface> {
        None
    }
    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        None
    }
    fn get_contain(&mut self) -> Option<&mut dyn ContainModuleInterface> {
        None
    }
    fn get_create(&mut self) -> Option<&mut dyn CreateModuleInterface> {
        None
    }
    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        None
    }
    fn get_destroy(&mut self) -> Option<&mut dyn DestroyModuleInterface> {
        None
    }
    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        None
    }
    fn get_special_power(&mut self) -> Option<&mut dyn SpecialPowerModuleInterface> {
        None
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        None
    }
    /// Optional flammability hook used by fire/ignite systems.
    /// PARITY_NOTE: C++ default is no-op; subclasses override when flammable.
    fn try_to_ignite_flammable(&mut self) {
        // Default: not flammable, nothing to do.
    }
    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        None
    }

    /// Specialized behavior interfaces
    fn get_parking_place_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn ParkingPlaceBehaviorInterface> {
        None
    }
    fn get_rebuild_hole_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn RebuildHoleBehaviorInterface> {
        None
    }
    fn get_bridge_behavior_interface(&mut self) -> Option<&mut dyn BridgeBehaviorInterface> {
        None
    }
    fn get_bridge_tower_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn BridgeTowerBehaviorInterface> {
        None
    }
    fn get_bridge_scaffold_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn BridgeScaffoldBehaviorInterface> {
        None
    }
    fn get_overcharge_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn OverchargeBehaviorInterface> {
        None
    }
    fn get_transport_passenger_interface(
        &mut self,
    ) -> Option<&mut dyn TransportPassengerInterface> {
        None
    }
    fn get_cave_interface(&mut self) -> Option<&mut dyn CaveInterface> {
        None
    }
    fn get_land_mine_interface(&mut self) -> Option<&mut dyn LandMineInterface> {
        None
    }
    fn get_eject_pilot_die_interface(&mut self) -> Option<&mut dyn DieModuleInterface> {
        None
    }
    fn get_countermeasures_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn CountermeasuresBehaviorInterface> {
        None
    }
    fn get_countermeasures_behavior_interface_const(
        &self,
    ) -> Option<&dyn CountermeasuresBehaviorInterface> {
        None
    }

    /// Update behavior interfaces
    fn get_projectile_update_interface(&mut self) -> Option<&mut dyn ProjectileUpdateInterface> {
        None
    }
    fn get_ai_update_interface(&mut self) -> Option<&mut dyn AIUpdateInterface> {
        None
    }
    fn get_update_exit_interface(&mut self) -> Option<&mut dyn ExitInterface> {
        None
    }
    fn get_dock_update_interface(&mut self) -> Option<&mut dyn DockUpdateInterface> {
        None
    }
    fn get_railed_transport_dock_update_interface(
        &mut self,
    ) -> Option<&mut dyn RailedTransportDockUpdateInterface> {
        None
    }
    fn get_slow_death_behavior_interface(&mut self) -> Option<&mut dyn SlowDeathBehaviorInterface> {
        None
    }
    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        None
    }
    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerModuleInterface> {
        None
    }
    fn get_special_power_module_interface_const(&self) -> Option<&dyn SpecialPowerModuleInterface> {
        None
    }
    fn get_ocl_update_interface(&mut self) -> Option<&mut dyn OCLUpdateInterface> {
        None
    }
    fn get_spy_vision_update(&mut self) -> Option<&mut dyn SpyVisionUpdate> {
        None
    }
    fn get_slaved_update_interface(&mut self) -> Option<&mut dyn SlavedUpdateInterface> {
        None
    }
    fn get_production_update_interface(&mut self) -> Option<&mut dyn ProductionUpdateInterface> {
        None
    }
    fn get_horde_update_interface(&mut self) -> Option<&mut dyn HordeUpdateInterface> {
        None
    }
    fn get_power_plant_update_interface(&mut self) -> Option<&mut dyn PowerPlantUpdateInterface> {
        None
    }
    fn get_spawn_behavior_interface(&mut self) -> Option<&mut dyn SpawnBehaviorInterface> {
        None
    }

    fn get_spawn_behavior_full_interface(
        &mut self,
    ) -> Option<&mut dyn crate::object::behavior::spawn_behavior::SpawnBehaviorInterface> {
        None
    }
    fn get_assisted_targeting_update_interface(
        &mut self,
    ) -> Option<&mut dyn AssistedTargetingUpdateInterface> {
        None
    }
    fn get_cleanup_hazard_update_interface(
        &mut self,
    ) -> Option<&mut dyn CleanupHazardUpdateInterface> {
        None
    }
    fn get_radius_decal_update_interface(&mut self) -> Option<&mut dyn RadiusDecalUpdateInterface> {
        None
    }
    fn get_projectile_stream_update_interface(
        &mut self,
    ) -> Option<&mut dyn ProjectileStreamUpdateInterface> {
        None
    }
    fn get_laser_behavior_control_interface(
        &mut self,
    ) -> Option<&mut dyn LaserBehaviorControlInterface> {
        None
    }
}

/// Interface for AssistedTargetingUpdate (matching C++ logic)
pub trait AssistedTargetingUpdateInterface {
    fn is_free_to_assist(&self) -> bool;
    fn assist_attack(&mut self, requesting_object_id: ObjectID, victim_object_id: ObjectID);
}

impl fmt::Debug for dyn BodyModuleInterface + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BodyModuleInterface")
    }
}

/// Base trait for behavior modules (matching C++ BehaviorModule)
pub trait BehaviorModule: BehaviorModuleInterface + std::fmt::Debug {
    /// Initialize the behavior
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Called when the object is destroyed
    fn on_destroy(&mut self);
}

pub trait ToppleControlInterface {
    fn is_able_to_be_toppled(&self) -> bool;
    fn apply_toppling_force(
        &mut self,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    );
    fn apply_toppling_force_with_object(
        &mut self,
        obj: &mut crate::object::Object,
        object_arc: &Arc<RwLock<crate::object::Object>>,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    );
}

/// Interface exposed by behaviors that manage timed object-creation lists.
pub trait OCLUpdateInterface: Send + Sync {
    fn reset_timer(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_remaining_frames(&self) -> Option<UnsignedInt> {
        None
    }

    fn get_countdown_percent(&self) -> Option<f32> {
        None
    }
}
