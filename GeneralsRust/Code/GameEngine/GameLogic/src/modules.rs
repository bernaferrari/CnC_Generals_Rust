//! Module interfaces and implementations
//!
//! This module provides all the module interfaces that objects use,
//! matching the C++ module system architecture.

use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType, GuardMode};
use crate::common::audio::AudioEventRts;
use crate::common::*;
use crate::damage::{DamageInfo, DamageInfoInput};
use crate::helpers::TheGameLogic;
use crate::object::behavior::behavior_module::SpyVisionUpdate;
use crate::object::behavior::behavior_module::{
    BridgeBehaviorInterface, BridgeScaffoldBehaviorInterface, BridgeTowerBehaviorInterface,
    CaveInterface, LandMineInterface, OverchargeBehaviorInterface, ParkingPlaceBehaviorInterface,
    RebuildHoleBehaviorInterface, TransportPassengerInterface,
};
use crate::object::behavior::laser_update::LaserBehaviorControlInterface;
use crate::object::behavior::projectile_stream_update::ProjectileStreamUpdateInterface;
use crate::object::behavior::radius_decal_update::RadiusDecalUpdateInterface;
use crate::object::registry::OBJECT_REGISTRY;
pub use crate::object::update::special_power_update::SpecialPowerCommandOption;
pub type SpecialPowerCommandOptions = SpecialPowerCommandOption;
use crate::command_button::CommandButton;
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::object::special_power_module::Waypoint;
use crate::object::update::ai_update::deliver_payload_data::DeliverPayloadData;
use crate::object::SpecialPowerTemplate;
use crate::player::PlayerIndex;
use game_engine::common::system::Xfer;
use std::any::Any;
/// Destroy reasons used by die/destroy modules (mirrors C++ DestroyReason).
#[derive(Debug, Clone, Copy)]
pub enum DestroyReason {
    Logical,
    Damage,
    Script,
}
use crate::locomotor::Locomotor;
use crate::object::body::body_module::ArmorSetType as BodyArmorSetType;
pub use crate::object::body::body_module::BodyModuleInterface;
use crate::object::Object;
pub use game_engine::common::thing::module::UpgradeMuxData;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

/// AI Attitude types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AIAttitudeType {
    Normal,
    Aggressive,
    Defensive,
    Passive,
    Sleep,
}

// Forward declarations for convenience
pub use crate::ai::group::AIGroup;
pub use crate::player::Player;
pub use crate::team::Team;
pub use crate::weapon::{Weapon, WeaponSet};

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

/// Contain module interface for garrison/transport (matching C++ ContainModuleInterface)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainWant {
    WantsToEnter,
    WantsToExit,
    WantsNeither,
}

pub trait ContainModuleInterface: Send + Sync + std::fmt::Debug {
    fn can_contain(&self, object_id: ObjectID) -> bool;
    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String>;
    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String>;
    fn get_contained_objects(&self) -> &[ObjectID];
    fn get_contained_count(&self) -> usize;
    fn get_max_capacity(&self) -> usize;

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Per-frame containment update.
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UpdateSleepTime::Forever)
    }

    /// Called after the owning object finishes module construction.
    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner damage.
    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Containment reaction to owner death.
    fn on_die(
        &mut self,
        _damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this container encloses (hides/masks) the given contained object.
    ///
    /// C++ reference: `ContainModuleInterface::isEnclosingContainerFor`.
    ///
    /// Most containers enclose their passengers; specialized riders (e.g. Overlord/Helix payloads)
    /// are expected to override this to return `false` for visible riders.
    fn is_enclosing_container_for(&self, _obj: &Object) -> bool {
        true
    }

    /// Whether this container is a heal-only container (matches C++ isHealContain).
    fn is_heal_contain(&self) -> bool {
        false
    }

    /// Whether this container can be busted by bunker buster weapons.
    fn is_bustable(&self) -> bool {
        false
    }

    /// Check if this container is valid for the given object
    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Default implementation - always allow
        let _ = (obj, check_capacity);
        true
    }

    /// Add object to containment
    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = obj.get_id();
        if !self.can_contain(object_id) {
            return Err("Container cannot accept object".into());
        }
        self.contain_object(object_id).map_err(|err| err.into())
    }

    /// Enable or disable load sounds for this container
    fn enable_load_sounds(
        &mut self,
        _enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Default implementation does nothing
        Ok(())
    }

    /// Notify container that an object wants to enter/exit.
    fn on_object_wants_to_enter_or_exit(
        &mut self,
        _obj: &Object,
        _want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this container can be garrisoned (default: false).
    fn is_garrisonable(&self) -> bool {
        false
    }

    /// Whether clear-building attacks should spare passengers.
    ///
    /// C++ parity: OpenContain defaults this to true; GarrisonContain overrides from INI.
    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    /// Attempt to position a garrisoned unit at the best fire point for a target object.
    /// Matches C++ ContainModuleInterface::attemptBestFirePointPosition.
    fn attempt_best_fire_point_position(
        &mut self,
        _source: Arc<RwLock<Object>>,
        _weapon: &crate::weapon::Weapon,
        _victim: Arc<RwLock<Object>>,
    ) -> bool {
        false
    }

    /// Attempt to position a garrisoned unit at the best fire point for a target position.
    /// Matches C++ ContainModuleInterface::attemptBestFirePointPosition (position overload).
    fn attempt_best_fire_point_position_coord(
        &mut self,
        _source: Arc<RwLock<Object>>,
        _weapon: &crate::weapon::Weapon,
        _target_pos: &Coord3D,
    ) -> bool {
        false
    }

    /// Returns the apparent controlling player when the container is garrisoned/stealth-contained.
    fn get_apparent_controlling_player(
        &self,
        _observing_player: Option<&Player>,
    ) -> Option<Arc<RwLock<Player>>> {
        None
    }

    /// Override drop destination for contained objects (default: no-op).
    fn set_override_destination(&mut self, _pos: &Coord3D) {}

    /// Set a rally point for contained units that exit this container.
    fn set_rally_point(&mut self, _pos: Coord3D) {}

    /// Return the rally point for contained units that exit this container.
    fn get_rally_point(&self) -> Option<Coord3D> {
        None
    }

    /// Whether the specified contained object can exit through this container.
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.get_contained_objects().contains(&object_id)
    }

    /// Reserve an exit door/path for a contained object.
    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&crate::object::Object>,
        _spawn: Option<&crate::object::Object>,
    ) -> ExitDoorType {
        DOOR_NONE_AVAILABLE
    }

    /// Release a reserved exit door/path.
    fn unreserve_door_for_exit(&mut self, _door: ExitDoorType) {}

    /// Exit a contained object via a reserved door/path.
    fn exit_object_via_door(
        &mut self,
        _obj: &Arc<RwLock<crate::object::Object>>,
        _door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Special tunnel-network style exit that preserves the passenger's current AI state.
    fn exit_object_in_a_hurry(
        &mut self,
        _obj: &Arc<RwLock<crate::object::Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether a passenger is allowed to fire (default: false).
    fn is_passenger_allowed_to_fire(&self, _id: Option<ObjectID>) -> bool {
        false
    }

    /// Whether the container passes weapon bonus flags to passengers.
    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        false
    }

    /// Toggle whether passengers may fire from this container (default: no-op).
    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        let _ = allowed;
    }

    /// Script hook for cave/tunnel containers (default: no-op).
    fn try_to_set_cave_index(&mut self, _new_index: Int) {}

    /// Script hook for garrison evac disposition (default: no-op).
    fn set_evac_disposition(&mut self, _disposition: UnsignedInt) {}

    /// Order all passengers to exit (matches C++ OpenContain::orderAllPassengersToExit).
    fn order_all_passengers_to_exit(
        &mut self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cmd = if instantly {
            AiCommandType::ExitInstantly
        } else {
            AiCommandType::Exit
        };

        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                let container_id = obj.read().ok().and_then(|guard| guard.get_contained_by());
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let mut params = AiCommandParams::new(cmd, command_source);
                            params.obj = container_id;
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Order all passengers to idle (matches C++ OpenContain::orderAllPassengersToIdle).
    fn order_all_passengers_to_idle(
        &mut self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let params = AiCommandParams::new(AiCommandType::Idle, command_source);
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Order money hackers to begin hacking (matches C++ OpenContain::orderAllPassengersToHackInternet).
    fn order_all_passengers_to_hack_internet(
        &mut self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for object_id in self.get_contained_objects() {
            if let Some(obj) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(obj_guard) = obj.read() {
                    if !obj_guard.is_kind_of(KindOf::Hacker) {
                        continue;
                    }
                    if let Some(ai) = obj_guard.get_ai() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            let params =
                                AiCommandParams::new(AiCommandType::HackInternet, command_source);
                            let _ = ai_guard.execute_command(&params);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Notify container that an object started being contained.
    fn on_containing(
        &mut self,
        _obj: Arc<RwLock<Object>>,
        _was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Notify container that an object is being removed.
    fn on_removing(
        &mut self,
        _obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Notify container that its owning object has been captured.
    /// Matches C++ `ContainModuleInterface::onCapture`.
    fn on_capture(
        &mut self,
        _owner: &Object,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        _new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Remove all contained objects.
    fn remove_all_contained(
        &mut self,
        _expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this contain should be displayed on the control bar.
    fn is_displayed_on_control_bar(&self) -> bool {
        true
    }

    /// Whether passengers should be kicked out on capture.
    fn is_kick_out_on_capture(&self) -> bool {
        true
    }

    /// Force all contained objects to exit, and damage them.
    /// Matches C++ OpenContain::harmAndForceExitAllContained.
    fn harm_and_force_exit_all_contained(
        &mut self,
        _damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ OpenContain::killAllContained.
    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Flash visible contained units as selected.
    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get contain count as u32 (matches legacy API).
    fn get_contain_count(&self) -> u32 {
        self.get_contained_count() as u32
    }

    /// Number of passengers currently hidden by stealth containment.
    fn get_stealth_units_contained(&self) -> UnsignedInt {
        self.get_contained_objects()
            .iter()
            .filter_map(|id| TheGameLogic::find_object_by_id(*id))
            .filter(|obj| {
                obj.read()
                    .ok()
                    .map(|guard| guard.test_status(ObjectStatusTypes::Stealthed))
                    .unwrap_or(false)
            })
            .count() as UnsignedInt
    }

    /// Get contain max as i32 (matches legacy API).
    fn get_contain_max(&self) -> i32 {
        let max = self.get_max_capacity();
        if max == usize::MAX {
            -1
        } else {
            max as i32
        }
    }

    /// Return the player mask for the last player who entered this container.
    fn get_player_who_entered(&self) -> PlayerMaskType {
        PlayerMaskType::none()
    }

    /// Return the special rider object for Overlord-style containers.
    fn friend_get_rider(&self) -> Option<ObjectID> {
        None
    }

    /// Whether any contained object wants to enter/exit (C++ hasObjectsWantingToEnterOrExit).
    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        false
    }

    /// Whether this container is a special Overlord-style container.
    fn is_special_overlord_style_container(&self) -> bool {
        false
    }

    /// Get the rider ID for Overlord-style containers (alias for friend_get_rider).
    fn get_rider_id(&self) -> Option<ObjectID> {
        self.friend_get_rider()
    }
}

/// Extension trait for Arc<Mutex<dyn ContainModuleInterface>> to provide convenient methods
pub trait ContainModuleInterfaceExt {
    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool;
    fn add_to_contain(&self, obj: &Object);
    fn get_contained_objects(&self) -> Vec<ObjectID>;
    fn get_contained_count(&self) -> usize;
    fn enable_load_sounds(
        &self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn on_removing(&self, obj: &Object);
    fn on_object_wants_to_enter_or_exit(&self, obj: &Object, want: ContainWant);
    fn is_enclosing_container_for(&self, obj: &Object) -> bool;
    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool;
    fn set_override_destination(&self, pos: &Coord3D);
    fn set_rally_point(&self, pos: Coord3D);
    fn get_rally_point(&self) -> Option<Coord3D>;
    fn has_objects_wanting_to_enter_or_exit(&self) -> bool;
    fn is_special_overlord_style_container(&self) -> bool;
    fn get_rider_id(&self) -> Option<ObjectID>;
    fn friend_get_rider(&self) -> Option<ObjectID>;
    fn order_all_passengers_to_exit(
        &self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_all_passengers_to_idle(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn order_all_passengers_to_hack_internet(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn harm_and_force_exit_all_contained(
        &self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn kill_all_contained(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

impl ContainModuleInterfaceExt for Arc<Mutex<dyn ContainModuleInterface>> {
    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_valid_container_for(obj, check_capacity)
        } else {
            false
        }
    }

    fn add_to_contain(&self, obj: &Object) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.add_to_contain(obj);
        }
    }

    fn get_contained_objects(&self) -> Vec<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_contained_objects().to_vec()
        } else {
            Vec::new()
        }
    }

    fn get_contained_count(&self) -> usize {
        if let Ok(guard) = self.try_lock() {
            guard.get_contained_count()
        } else {
            0
        }
    }

    fn enable_load_sounds(
        &self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.enable_load_sounds(enabled)
        } else {
            Err("Failed to lock ContainModuleInterface".into())
        }
    }

    fn on_removing(&self, _obj: &Object) {
        if let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(_obj.get_id()) {
            if let Ok(mut guard) = self.try_lock() {
                let _ = guard.on_removing(obj_arc);
            }
        }
    }

    fn on_object_wants_to_enter_or_exit(&self, obj: &Object, want: ContainWant) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.on_object_wants_to_enter_or_exit(obj, want);
        }
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_enclosing_container_for(obj)
        } else {
            true
        }
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_passenger_allowed_to_fire(id)
        } else {
            false
        }
    }

    fn set_override_destination(&self, pos: &Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_override_destination(pos);
        }
    }

    fn set_rally_point(&self, pos: Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_rally_point(pos);
        }
    }

    fn get_rally_point(&self) -> Option<Coord3D> {
        self.try_lock()
            .ok()
            .and_then(|guard| guard.get_rally_point())
    }

    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.has_objects_wanting_to_enter_or_exit()
        } else {
            false
        }
    }

    fn is_special_overlord_style_container(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_special_overlord_style_container()
        } else {
            false
        }
    }

    fn get_rider_id(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_rider_id()
        } else {
            None
        }
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.friend_get_rider()
        } else {
            None
        }
    }

    fn order_all_passengers_to_exit(
        &self,
        command_source: CommandSourceType,
        instantly: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_exit(command_source, instantly)
        } else {
            Ok(())
        }
    }

    fn order_all_passengers_to_idle(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_idle(command_source)
        } else {
            Ok(())
        }
    }

    fn order_all_passengers_to_hack_internet(
        &self,
        command_source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.order_all_passengers_to_hack_internet(command_source)
        } else {
            Ok(())
        }
    }

    fn harm_and_force_exit_all_contained(
        &self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.harm_and_force_exit_all_contained(damage_info)
        } else {
            Ok(())
        }
    }

    fn kill_all_contained(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.kill_all_contained()
        } else {
            Ok(())
        }
    }
}

/// AI update interface (matching C++ AIUpdateInterface)
pub trait AIUpdateInterface: Send + Sync + std::fmt::Debug {
    /// Update AI logic
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Check if the object is moving
    fn is_moving(&self) -> bool;
    /// Check if the object is idle
    fn is_idle(&self) -> bool;
    /// Check if the object is idle without pending-command suppression.
    fn is_idle_unrestricted(&self) -> bool {
        self.is_idle()
    }
    /// Check if the object is currently attacking.
    fn is_attacking(&self) -> bool {
        false
    }
    /// Set movement target
    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String>;
    /// Returns the locomotor preferred height if available.
    fn get_preferred_height(&self) -> Option<Real> {
        self.get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|guard| guard.preferred_height))
    }

    /// Get current locomotor (matches C++ AIUpdateInterface::getCurLocomotor).
    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        None
    }
    /// Get whether a locomotor path is active (matches C++ AIUpdateInterface::getPath).
    fn get_path(&self) -> Option<()> {
        self.get_path_destination().map(|_| ())
    }
    /// Get the destination of the active path (matches C++ AIUpdateInterface::getPath last node).
    fn get_path_destination(&self) -> Option<Coord3D> {
        None
    }
    /// Get remaining distance to goal along locomotor path (matches C++ getLocomotorDistanceToGoal).
    fn get_locomotor_distance_to_goal(&self) -> Real {
        0.0
    }

    /// Get current enter/garrison target (matches C++ AIUpdateInterface::getEnterTarget).
    fn get_enter_target(&self) -> Option<ObjectID> {
        None
    }
    /// Get last command source
    fn get_last_command_source(&self) -> crate::ai::CommandSourceType {
        crate::ai::CommandSourceType::FromAi // Default to AI source
    }
    /// Set last command source (matches C++ friend_setLastCommandSource).
    fn set_last_command_source(&mut self, _source: crate::ai::CommandSourceType) {}
    /// Get current AI command (matches C++ AIUpdateInterface::getAIStateType for docking checks).
    fn get_current_command(&self) -> Option<AiCommandType> {
        None
    }
    /// Get pending AI command (matches C++ AIUpdateInterface::friend_getPendingCommandType).
    fn get_pending_command_type(&self) -> Option<AiCommandType> {
        None
    }
    /// Purge pending AI command (matches C++ AIUpdateInterface::friend_purgePendingCommand).
    fn purge_pending_command(&mut self) {}

    /// Clear locomotor goal (matches C++ AIUpdateInterface::setLocomotorGoalNone).
    fn set_locomotor_goal_none(&mut self) {
        self.destroy_path();
    }

    /// Set locomotor goal orientation (matches C++ AIUpdateInterface::setLocomotorGoalOrientation).
    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        let _ = angle;
    }

    /// Set locomotor goal position explicitly (matches C++ AIUpdateInterface::setLocomotorGoalPositionExplicit).
    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        let _ = pos;
    }

    /// Notify AI that a move is ending (matches C++ friend_endingMove).
    fn friend_ending_move(&mut self) {
        let _ = self.is_blocked_and_stuck();
    }
    /// Notify AI that a move is starting (matches C++ friend_startingMove).
    fn friend_starting_move(&mut self) {
        self.set_blocked_and_stuck(false);
    }

    /// Set surrendered state (matches C++ AIUpdateInterface::setSurrendered).
    fn set_surrendered(&mut self, to_object: Option<&Arc<RwLock<Object>>>, surrendered: bool) {
        let _ = (to_object, surrendered);
    }

    /// Check if this unit is surrendered (matches C++ AIUpdateInterface::isSurrendered).
    fn is_surrendered(&self) -> bool {
        false
    }

    /// Player index we surrendered to, if any (matches C++ AIUpdateInterface::getSurrenderedPlayerIndex).
    fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        None
    }

    /// Whether the AI is allowed to adjust destination on the fly.
    fn is_allowed_to_adjust_destination(&self) -> bool {
        true
    }

    /// Whether this aircraft should adjust destination (matches isAircraftThatAdjustsDestination).
    fn is_aircraft_that_adjusts_destination(&self) -> bool {
        false
    }

    /// Desired movement speed (matches C++ AIUpdateInterface::getDesiredSpeed).
    fn get_desired_speed(&self) -> Real {
        FAST_AS_POSSIBLE
    }

    /// Set desired movement speed (matches C++ AIUpdateInterface::setDesiredSpeed).
    fn set_desired_speed(&mut self, speed: Real) {
        let _ = speed;
    }

    /// Whether the unit is currently rappelling (AI_RAPPEL_INTO state support).
    fn is_in_rappel_state(&self) -> bool {
        false
    }

    /// Whether the unit is currently performing a combat drop (chinook AI support).
    fn is_doing_combat_drop(&self) -> bool {
        false
    }

    /// Whether this unit is moving out of the way of another unit (matches C++ isMovingAwayFrom).
    fn is_moving_away_from(&self, _obj_id: ObjectID) -> bool {
        false
    }

    /// Set a duration to ignore collisions (matches C++ setIgnoreCollisionTime).
    fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        let _ = duration_frames;
    }

    /// Frame until which collisions should be ignored.
    fn get_ignore_collisions_until(&self) -> UnsignedInt {
        0
    }

    /// Queue a pathfinding request after a delay (matches AIUpdateInterface::setQueueForPathTime).
    fn set_queue_for_path_time(&mut self, _frames: UnsignedInt) {}

    /// Re-evaluate horde/nationalism/fanaticism morale bonuses (matches C++ evaluateMoraleBonus).
    fn evaluate_morale_bonus(&mut self) {
        let _ = self.get_current_victim();
    }

    /// Whether AI can move away from another unit (matches JetAIUpdate::isAllowedToMoveAwayFromUnit).
    fn is_allowed_to_move_away_from_unit(&self) -> bool {
        true
    }

    /// Provide a sneaky targeting offset (matches JetAIUpdate::getSneakyTargetingOffset).
    fn get_sneaky_targeting_offset(&self, _offset: &mut Coord3D) -> bool {
        false
    }

    /// Whether aim success is temporarily prevented (matches JetAIUpdate::isTemporarilyPreventingAimSuccess).
    fn is_temporarily_preventing_aim_success(&self) -> bool {
        false
    }

    /// Whether out of special return-to-base ammo (matches JetAIUpdate::isOutOfSpecialReloadAmmo).
    fn is_out_of_special_reload_ammo(&self) -> bool {
        false
    }

    /// Add or remove a targeter (matches C++ AIUpdateInterface::addTargeter).
    fn add_targeter(&mut self, _id: ObjectID, _add: bool) {}

    /// Whether turrets are linked (matches C++ AIUpdateInterface::areTurretsLinked).
    fn are_turrets_linked(&self) -> Bool {
        false
    }

    /// Set turret target object (matches C++ AIUpdateInterface::setTurretTargetObject).
    fn set_turret_target_object(
        &mut self,
        _turret: TurretType,
        _target: Option<&Arc<RwLock<Object>>>,
        _force_attacking: bool,
    ) {
    }

    /// Set turret target position (matches C++ AIUpdateInterface::setTurretTargetPosition).
    fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        self.set_turret_target_object(turret, None, false);
        let _ = pos;
    }

    /// Whether to treat as aircraft for distance-to-goal (matches JetAIUpdate::getTreatAsAircraftForLocoDistToGoal).
    fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        true
    }

    /// Whether a contained object is free to exit (matches C++ AIUpdateInterface::getAiFreeToExit).
    fn get_ai_free_to_exit(&self, _exiter: &Object) -> crate::object::production::AIFreeToExitType {
        crate::object::production::AIFreeToExitType::FreeToExit
    }

    /// Mark this unit as demoralized for a duration in frames.
    /// Matches C++ AIUpdateInterface::setDemoralized.
    fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        let _ = duration_frames;
    }

    /// Transfer active attackers from one object to another (matches C++ transferAttack).
    fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        let _ = (from_id, to_id);
    }

    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        _slot: crate::weapon::WeaponSlotType,
        _target: crate::common::ObjectID,
    ) -> bool {
        false
    }
    /// Check if dock is open
    fn is_dock_open(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
    /// Returns whether the unit is taxiing to its parking position (matches JetAIUpdate::isTaxiingToParking).
    fn is_taxiing_to_parking(&self) -> bool {
        false
    }
    /// Returns whether the unit is reloading (matches JetAIUpdate::isReloading).
    fn is_reloading(&self) -> bool {
        false
    }
    /// Returns whether the unit is clearing mines (matches AIUpdateInterface::isClearingMines).
    fn is_clearing_mines(&self) -> Bool {
        false
    }
    /// Returns whether the unit is taking off or landing (matches JetAIUpdate::isTakeoffOrLandingInProgress).
    fn is_takeoff_or_landing_in_progress(&self) -> bool {
        false
    }

    /// Current AI state ID, if available (matches AIStateMachine::getCurrentStateID usage).
    fn get_current_state_id(&self) -> Option<u32> {
        None
    }
    /// Returns parking offset for jets (matches JetAIUpdate::friend_getParkingOffset).
    fn get_parking_offset(&self) -> Real {
        0.0
    }
    /// Returns whether jets keep parking space while airborne (matches JetAIUpdate::friend_keepsParkingSpaceWhenAirborne).
    fn keeps_parking_space_when_airborne(&self) -> bool {
        true
    }
    /// Cancel dock operation
    fn cancel_dock(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    /// Supply truck AI interface access
    fn get_supply_truck_ai_interface(&self) -> Option<&dyn SupplyTruckAIInterface> {
        None
    }
    /// Mutable supply truck AI interface access
    fn get_supply_truck_ai_interface_mut(&mut self) -> Option<&mut dyn SupplyTruckAIInterface> {
        None
    }
    /// POW truck AI interface access
    fn get_pow_truck_ai_update_interface(&mut self) -> Option<&mut dyn POWTruckAIUpdateInterface> {
        None
    }
    /// Hack internet AI interface access
    fn get_hack_internet_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn HackInternetAIUpdateInterface> {
        None
    }
    /// Assault transport AI interface access
    fn get_assault_transport_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn AssaultTransportAIUpdateInterface> {
        None
    }
    /// Worker AI update interface access
    fn get_worker_ai_update_interface_mut(&mut self) -> Option<&mut dyn WorkerAIUpdateInterface> {
        None
    }
    /// Dozer AI update interface access
    fn get_dozer_ai_update_interface_mut(&mut self) -> Option<&mut dyn DozerAIUpdateInterface> {
        None
    }
    /// Deliver payload AI interface access
    fn get_deliver_payload_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn DeliverPayloadAIUpdateInterface> {
        None
    }
    fn ignore_obstacle(
        &mut self,
        _obj: Option<&Arc<RwLock<Object>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get current ignored obstacle ID (matches AIUpdateInterface::getIgnoredObstacleID).
    fn get_ignored_obstacle_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }

    /// Apply bump speed limit logic when blocked (matches AIUpdateInterface::doLocomotor).
    fn apply_bump_speed_limit(&mut self, desired_speed: Real, _blocked: bool) -> Real {
        desired_speed
    }

    /// Set the current goal path index (used by waypoint rendering/debug UI).
    fn set_current_goal_path_index(
        &mut self,
        _index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get the current goal path index (default -1).
    fn get_current_goal_path_index(&self) -> i32 {
        -1
    }

    /// Allow pathing through units (AIStates.cpp uses this in AIDockState).
    fn set_can_path_through_units(
        &mut self,
        _value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Whether this unit can path through other units (matches AIUpdateInterface::getCanPathThroughUnits).
    fn get_can_path_through_units(&self) -> bool {
        false
    }

    /// Check if the unit is blocked and stuck (MoveOutOfTheWay uses this).
    fn is_blocked_and_stuck(&self) -> bool {
        false
    }

    /// Mark this unit as blocked this frame (matches AIUpdateInterface::m_isBlocked).
    fn set_is_blocked(&mut self, blocked: bool) {
        if !blocked {
            self.set_blocked_and_stuck(false);
        }
    }

    /// Mark this unit as blocked and stuck (matches AIUpdateInterface::m_isBlockedAndStuck).
    fn set_blocked_and_stuck(&mut self, _blocked: bool) {}

    /// Frames blocked for movement (matches AIUpdateInterface::getNumFramesBlocked).
    fn get_num_frames_blocked(&self) -> u32 {
        0
    }

    /// Clear any active pathing state.
    fn destroy_path(&mut self) {
        self.set_goal_object(None);
    }

    /// Clear move-out-of-way state (noop by default).
    fn clear_move_out_of_way(&mut self) {}

    /// Get goal object (for AI coordination)
    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        None
    }

    /// Set goal object (friend_setGoalObject parity).
    fn set_goal_object(&mut self, obj: Option<&Arc<RwLock<Object>>>) {
        let _ = obj;
    }

    /// Check if any path exists to a destination (matches AIUpdateInterface::isPathAvailable).
    fn is_path_available(&self, _destination: &Coord3D) -> bool {
        false
    }

    /// Request a path to a destination (matches AIUpdateInterface::requestPath).
    fn request_path(&mut self, _destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        Ok(())
    }

    /// Request a path to attack a victim (matches AIUpdateInterface::requestAttackPath).
    fn request_attack_path(
        &mut self,
        _victim_id: ObjectID,
        _victim_pos: &Coord3D,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Request an approach path (matches AIUpdateInterface::requestApproachPath).
    fn request_approach_path(&mut self, _destination: &Coord3D) -> Result<(), String> {
        Ok(())
    }

    /// Whether we can compute a quick path (matches AIUpdateInterface::canComputeQuickPath).
    fn can_compute_quick_path(&self) -> bool {
        false
    }

    /// Compute a quick path to destination (matches AIUpdateInterface::computeQuickPath).
    fn compute_quick_path(&mut self, _destination: &Coord3D) -> bool {
        false
    }

    /// Check if a quick path exists to a destination (matches AIUpdateInterface::isQuickPathAvailable).
    fn is_quick_path_available(&self, _destination: &Coord3D) -> bool {
        false
    }

    /// Check if a locomotor position is valid (matches AIUpdateInterface::isValidLocomotorPosition).
    fn is_valid_locomotor_position(&self, _pos: &Coord3D) -> bool {
        false
    }

    /// Whether the unit needs to rotate to align with its path (matches AIUpdateInterface::needToRotate).
    fn need_to_rotate(&self) -> bool {
        false
    }

    /// Current locomotor set type (matches AIUpdateInterface::getCurLocomotorSetType).
    fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        LocomotorSetType::Invalid
    }

    /// Whether any locomotor supports the requested surface type.
    fn has_locomotor_for_surface(&self, _surface: crate::common::LocomotorSurfaceTypeMask) -> bool {
        false
    }

    /// Max locomotor speed for current damage condition (matches AIUpdateInterface::getCurLocomotorSpeed).
    fn get_cur_locomotor_speed(&self) -> Real {
        0.0
    }

    /// Current max blocked speed (matches AIUpdateInterface::m_curMaxBlockedSpeed).
    fn get_cur_max_blocked_speed(&self) -> Real {
        FAST_AS_POSSIBLE
    }

    /// Set current max blocked speed (matches AIUpdateInterface::m_curMaxBlockedSpeed).
    fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        let _ = speed;
    }
    /// Get current crate ID (matching C++ AIUpdateInterface::getCrateID)
    fn get_crate_id(&self) -> ObjectID {
        crate::common::INVALID_ID
    }
    /// Get current victim target (matching C++ AIUpdateInterface::getCurrentVictim).
    fn get_current_victim(&self) -> Option<ObjectID> {
        None
    }
    /// Set current victim target (matching C++ AIUpdateInterface::setCurrentVictim).
    fn set_current_victim(&mut self, _victim: Option<ObjectID>) {}
    /// Check for crate to pick up (matching C++ AIUpdateInterface::checkForCrateToPickup)
    fn check_for_crate_to_pickup(&self) -> Option<Arc<RwLock<Object>>> {
        None
    }
    /// Get next target based on mood/auto-acquire (matching C++ AIUpdateInterface::getNextMoodTarget)
    fn get_next_mood_target(
        &mut self,
        _use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        None
    }
    /// Get next mood check time (matching C++ AIUpdateInterface::getNextMoodCheckTime)
    fn get_next_mood_check_time(&self) -> u32 {
        TheGameLogic::get_frame()
    }
    /// Reset next mood check time (matching C++ AIUpdateInterface::resetNextMoodCheckTime)
    fn reset_next_mood_check_time(&mut self) {}
    /// Set next mood check time (matching C++ AIUpdateInterface::setNextMoodCheckTime)
    fn set_next_mood_check_time(&mut self, _frame: u32) {}
    /// Get packed mood matrix parameters (matching C++ AIUpdateInterface::getMoodMatrixValue).
    fn get_mood_matrix_value(&self) -> u32 {
        0
    }
    /// Mood matrix action adjustment (matching C++ AIUpdateInterface::getMoodMatrixActionAdjustment)
    fn get_mood_matrix_action_adjustment(&mut self, _action: crate::ai::MoodMatrixAction) -> u32 {
        0
    }

    /// Notify AI that a shot has been fired (matches C++ AIAttackState::notifyFired).
    fn notify_fired(&mut self) {
        let _ = self.is_in_attack_state();
    }

    /// Notify AI that a new victim was chosen (matches C++ AIAttackState::notifyNewVictimChosen).
    fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        let _ = victim;
    }

    /// Whether a given weapon slot is allowed to fire for this attack (matches C++ isWeaponSlotOkToFire).
    fn is_weapon_slot_ok_to_fire(&self, _wslot: crate::weapon::WeaponSlotType) -> Bool {
        true
    }

    /// Original victim position for attack continuation (matches AIAttackState::getOriginalVictimPos).
    fn get_original_victim_pos(&self) -> Option<Coord3D> {
        None
    }

    /// Set original victim position for attack continuation.
    fn set_original_victim_pos(&mut self, _pos: Option<Coord3D>) {}

    /// Whether the current AI state machine is in an attack state.
    fn is_in_attack_state(&self) -> bool {
        false
    }

    /// Whether the current AI state machine is in guard idle.
    fn is_in_guard_idle_state(&self) -> bool {
        false
    }

    /// Set a temporary AI state (matches AIStateMachine::setTemporaryState).
    fn set_temporary_state(&mut self, _state: AIStateType, _frame_limit: UnsignedInt) {}
    /// Notify AI about a crate created by this unit (matching C++ AIUpdateInterface::notifyCrate)
    fn notify_crate(&mut self, crate_id: ObjectID) {
        let _ = crate_id;
    }

    /// Notify AI that its current victim died (matches C++ AIUpdateInterface::notifyVictimIsDead).
    fn notify_victim_is_dead(&mut self) {
        self.set_goal_object(None);
    }
    /// Record prior waypoint ID (matching C++ setPriorWaypointID)
    fn set_prior_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {}
    /// Record current waypoint ID (matching C++ setCurrentWaypointID)
    fn set_current_waypoint_id(&mut self, _waypoint_id: crate::waypoint::WaypointId) {}
    /// Record completed waypoint (matching C++ setCompletedWaypoint)
    fn set_completed_waypoint_id(&mut self, _waypoint_id: Option<crate::waypoint::WaypointId>) {}
    /// Get most recently completed waypoint (matching C++ getCompletedWaypoint)
    fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        None
    }

    /// Check if clear to advance
    fn is_clear_to_advance(
        &self,
        _obj: &Arc<RwLock<Object>>,
        _approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Reserve approach position
    fn reserve_approach_position(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
        _goal_pos: &mut Coord3D,
        _approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Execute a command packet (matching C++ AIUpdateInterface::ExecuteCommand)
    fn execute_command(
        &mut self,
        _command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Set locomotor upgrade flag (matching C++ AIUpdateInterface::setLocomotorUpgrade).
    fn set_locomotor_upgrade(
        &mut self,
        _enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Allow AI-issued commands to chase targets (matching C++ AIUpdateInterface::setAllowedToChase).
    fn set_allow_chase(&mut self, allowed: bool) {
        let _ = allowed;
    }

    /// Select locomotor set (matching C++ AIUpdateInterface::chooseLocomotorSet).
    fn choose_locomotor_set(
        &mut self,
        _set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Allow invalid positions (matching C++ Locomotor::setAllowInvalidPosition).
    fn set_allow_invalid_position(
        &mut self,
        _allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Toggle ultra-accurate pathing (matching C++ Locomotor::setUltraAccurate).
    fn set_ultra_accurate(
        &mut self,
        _ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Toggle precise Z positioning for pathing (matching C++ Locomotor::setPreciseZPos).
    fn set_precise_z_pos(
        &mut self,
        _precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// On approach reached
    fn on_approach_reached(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Advance approach position
    fn advance_approach_position(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
        _goal_pos: &mut Coord3D,
        _approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Check if clear to enter
    fn is_clear_to_enter(
        &self,
        _obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(true)
    }

    /// Get enter position
    fn get_enter_position(
        &self,
        _obj: &Arc<RwLock<Object>>,
        _goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// On enter reached
    fn on_enter_reached(
        &mut self,
        _obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Get dock position
    fn get_dock_position(
        &self,
        _obj: &Arc<RwLock<Object>>,
        _goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Check if allow passthrough type
    fn is_allow_passthrough_type(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }

    /// Get speed
    fn get_speed(&self) -> f32 {
        1.0
    }

    /// AI move to position
    fn ai_move_to_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI move to and evacuate
    fn ai_move_to_and_evacuate(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI idle
    fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI hunt
    fn ai_hunt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI force attack object
    fn ai_force_attack_object(
        &mut self,
        _target: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI attack object
    fn ai_attack_object(
        &mut self,
        _target: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI attack position
    fn ai_attack_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI guard position
    fn ai_guard_position(
        &mut self,
        _pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// AI guard object
    fn ai_guard_object(
        &mut self,
        _target: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Trigger prone behavior (matching C++ AIUpdateInterface::privateGoProne)
    fn ai_go_prone(&mut self, _damage_info: &DamageInfo, _cmd_source: CommandSourceType) {
        // Default implementation does nothing
    }

    /// AI busy (matching C++ AIUpdateInterface::aiBusy)
    fn ai_busy(
        &mut self,
        _cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Check if busy
    fn is_busy(&self) -> bool {
        false
    }

    /// Set attitude
    fn set_attitude(
        &mut self,
        _attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Set recruitable state (matches C++ AIUpdateInterface::setIsRecruitable).
    fn set_is_recruitable(&mut self, _recruitable: Bool) {}

    /// Get attitude
    fn get_attitude(&self) -> AIAttitudeType {
        AIAttitudeType::Normal
    }

    /// Check if AI is in dead state (used by slow death behaviors)
    fn is_ai_in_dead_state(&self) -> bool {
        false // Default: AI is not in dead state
    }

    /// Mark AI as dead (prevents other behaviors from handling death)
    fn mark_as_dead(&mut self) {
        // Default implementation does nothing - subclasses should track this state
    }

    /// Get which turret is used for current weapon
    /// Matches C++ AIUpdateInterface::GetWhichTurretForCurWeapon
    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        TurretType::Invalid
    }

    /// Get which turret is used for a weapon slot
    /// Matches C++ AIUpdateInterface::GetWhichTurretForWeaponSlot
    fn get_which_turret_for_weapon_slot(&self, _slot: crate::weapon::WeaponSlotType) -> TurretType {
        TurretType::Invalid
    }

    /// Set turret enabled state
    /// Matches C++ AIUpdateInterface::SetTurretEnabled
    fn set_turret_enabled(&mut self, _turret: TurretType, _enabled: bool) {
        // Default implementation does nothing
    }

    /// Recenter turret to natural position
    /// Matches C++ AIUpdateInterface::RecenterTurret
    fn recenter_turret(&mut self, _turret: TurretType) {
        // Default implementation does nothing
    }

    /// Set extra distance used when following paths (matches AIUpdateInterface::setPathExtraDistance)
    fn set_path_extra_distance(
        &mut self,
        _distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Build a path from a waypoint chain (matches AIUpdateInterface::setPathFromWaypoint).
    fn set_path_from_waypoint(
        &mut self,
        _waypoint: &crate::waypoint::Waypoint,
        _group_offset: &Coord2D,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Check whether the waypoint queue is empty (used by waypoint path exact follow).
    fn is_waypoint_queue_empty(&self) -> bool {
        true
    }

    /// Check if AI is waiting for path (matches AIUpdateInterface::isWaitingForPath).
    fn is_waiting_for_path(&self) -> bool {
        false
    }

    /// Append a goal position to the current path (used for off-map movement).
    fn append_goal_position_to_path(&mut self, _goal: &Coord3D) -> Result<(), String> {
        Ok(())
    }

    /// Replace current path with an explicit waypoint list (legacy-safe path support).
    fn set_path_from_coords(&mut self, _path: &[Coord3D]) -> Result<(), String> {
        Ok(())
    }

    /// Request a safe path away from a repulsor object (matches AIUpdateInterface::requestSafePath).
    fn request_safe_path(&mut self, _repulsor_id: ObjectID) -> Result<bool, String> {
        Ok(false)
    }

    /// Returns whether current movement is ground-based (matches AIUpdateInterface::isDoingGroundMovement).
    fn is_doing_ground_movement(&self) -> bool {
        true
    }

    /// Update pathfinding goal position for this unit.
    fn update_goal_position(
        &mut self,
        _goal: &Coord3D,
        _layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Adjust destination to a nearby passable location (returns false if no adjustment found).
    fn adjust_destination(&mut self, _goal: &mut Coord3D) -> bool {
        true
    }

    /// Set whether path following should adjust destinations (matches AIInternalMoveToState logic).
    fn set_adjusts_destination(&mut self, adjust: bool) {
        let _ = adjust;
    }

    /// Check if turret is in natural position
    /// Matches C++ AIUpdateInterface::IsTurretInNaturalPosition
    fn is_turret_in_natural_position(&self, _turret: TurretType) -> bool {
        true // Default: assume turret is centered
    }

    /// Check if turret is enabled (matches C++ AIUpdateInterface::isTurretEnabled).
    fn is_turret_enabled(&self, _turret: TurretType) -> bool {
        true
    }

    /// Get turret rotation and pitch (matches C++ AIUpdateInterface::getTurretRotAndPitch).
    fn get_turret_rot_and_pitch(&self, _turret: TurretType) -> Option<(Real, Real)> {
        None
    }

    /// Turret angle in radians (matches C++ AIUpdateInterface::getTurretAngle).
    fn get_turret_angle(&self, _turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(_turret)
            .map(|(angle, _)| angle)
            .unwrap_or(0.0)
    }

    /// Turret pitch in radians (matches C++ AIUpdateInterface::getTurretPitch).
    fn get_turret_pitch(&self, _turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(_turret)
            .map(|(_, pitch)| pitch)
            .unwrap_or(0.0)
    }

    /// C++ parity: AIUpdateInterface::queueWaypoint() — store waypoint without starting execution
    fn queue_waypoint(&mut self, _pos: &Coord3D) {}

    /// C++ parity: AIUpdateInterface::executeWaypointQueue() — start the first queued waypoint
    fn execute_waypoint_queue(&mut self) {}
}

/// Extension trait for Arc<Mutex<dyn AIUpdateInterface>> to provide convenient methods
pub trait AIUpdateInterfaceExt {
    fn get_speed(&self) -> f32;
    fn ai_move_to_position(&self, pos: &Coord3D, add_waypoint: bool, cmd_source: CommandSourceType);
    fn ai_move_to_position_even_if_sleeping(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_object(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_tighten_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_to_and_evacuate_and_exit(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_idle(&self, cmd_source: CommandSourceType);
    fn ai_hunt(&self, cmd_source: CommandSourceType);
    fn ai_busy(&self, cmd_source: CommandSourceType);
    fn ai_enter(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_force_attack_object(
        &self,
        victim: &Arc<RwLock<Object>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object(
        &self,
        victim: &Arc<RwLock<Object>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_object_id(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_move_to_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_team(
        &self,
        team: &Arc<RwLock<Team>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_attack_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_waypoint_path_exact_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_exit_production_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    );
    fn ai_follow_path_append(&self, pos: &Coord3D, cmd_source: CommandSourceType);
    fn ai_move_away_from_unit(&self, obj_id: ObjectID, cmd_source: CommandSourceType);
    fn ai_guard_retaliate(
        &self,
        victim: &Arc<RwLock<Object>>,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_position(
        &self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
    fn ai_guard_object(
        &self,
        obj_to_guard: &Arc<RwLock<Object>>,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    );
    fn is_idle(&self) -> bool;
    fn is_busy(&self) -> bool {
        !self.is_idle()
    }
    fn set_attitude(&self, attitude: AIAttitudeType);
    fn get_attitude(&self) -> AIAttitudeType;
    fn is_ai_in_dead_state(&self) -> bool;
    fn mark_as_dead(&self);
    fn get_last_command_source(&self) -> CommandSourceType;
    fn get_which_turret_for_cur_weapon(&self) -> TurretType;
    fn set_turret_enabled(&self, turret: TurretType, enabled: bool);
    fn recenter_turret(&self, turret: TurretType);
    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool;
    fn get_path(&self) -> Option<()>;
    fn get_path_destination(&self) -> Option<Coord3D>;
    fn get_locomotor_distance_to_goal(&self) -> Real;
    fn get_current_victim(&self) -> Option<ObjectID>;
    fn set_current_victim(&mut self, victim: Option<ObjectID>);
    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>>;
    fn get_preferred_height(&self) -> Option<Real>;
    fn ai_go_prone(&self, damage_info: &DamageInfo, cmd_source: CommandSourceType);
    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>>;
    fn choose_locomotor_set(&self, set: LocomotorSetType);
    fn set_allow_invalid_position(&self, allow: bool);
    fn set_ultra_accurate(&self, ultra: bool);
    fn execute_command(
        &self,
        params: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn queue_waypoint(&self, pos: &Coord3D);
    fn execute_waypoint_queue(&self);
}

impl AIUpdateInterfaceExt for Arc<Mutex<dyn AIUpdateInterface>> {
    fn get_speed(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_speed()
        } else {
            0.0
        }
    }

    fn ai_move_to_position(
        &self,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                if add_waypoint {
                    crate::ai::AiCommandType::FollowPathAppend
                } else {
                    crate::ai::AiCommandType::MoveToPosition
                },
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_position_even_if_sleeping(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionEvenIfSleeping,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_object(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::MoveToObject, cmd_source);
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_tighten_to_position(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::TightenToPosition,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_and_evacuate(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuate()
        // Move to position and then evacuate (exit garrison/transport)
        if let Ok(mut guard) = self.try_lock() {
            // Issue move-to-and-evacuate command
            // The AI state machine handles the evacuation after move completes
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionAndEvacuate,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_to_and_evacuate_and_exit(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiMoveToAndEvacuateAndExit()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_idle(&self, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Idle, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_hunt(&self, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Hunt, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_busy(&self, cmd_source: CommandSourceType) {
        // C++ Reference: AIUpdateInterface::aiBusy()
        if let Ok(mut guard) = self.try_lock() {
            let params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Busy, cmd_source);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_enter(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Enter, cmd_source);
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_force_attack_object(
        &self,
        victim: &Arc<RwLock<Object>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiForceAttackObject()
        // Force attack ignores normal targeting restrictions
        if let Ok(mut guard) = self.try_lock() {
            // Get victim's object ID
            if let Ok(victim_guard) = victim.read() {
                let victim_id = victim_guard.get_id();
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::ForceAttackObject,
                    cmd_source,
                );
                params.obj = Some(victim_id);
                params.int_value = max_shots_to_fire;
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn ai_attack_object(
        &self,
        victim: &Arc<RwLock<Object>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackObject()
        // Normal attack follows targeting rules
        if let Ok(mut guard) = self.try_lock() {
            // Get victim's object ID
            if let Ok(victim_guard) = victim.read() {
                let victim_id = victim_guard.get_id();
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::AttackObject,
                    cmd_source,
                );
                params.obj = Some(victim_id);
                params.int_value = max_shots_to_fire;
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn ai_attack_object_id(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackObject() with object ID
        if victim_id == INVALID_ID || OBJECT_REGISTRY.get_object(victim_id).is_none() {
            return;
        }
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
            params.obj = Some(victim_id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_move_to_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackMoveToPosition()
        // From AIStates.cpp: AI_ATTACK_MOVE_TO state
        // This is a special movement mode where the unit moves to a destination
        // but engages enemies encountered along the way (unlike regular move which ignores enemies)
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackMoveToPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_team(
        &self,
        team: &Arc<RwLock<Team>>,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackTeam()
        if let Ok(team_guard) = team.read() {
            if let Ok(mut guard) = self.try_lock() {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::AttackTeam,
                    cmd_source,
                );
                params.team = Some(team_guard.get_name().as_str().to_string());
                params.int_value = max_shots_to_fire;
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn ai_attack_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPath()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackFollowWaypointPath,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_attack_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiAttackFollowWaypointPathAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            params.int_value = max_shots_to_fire;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPath()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPath,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_exact(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExact()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathExact,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathAsTeam,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_waypoint_path_exact_as_team(
        &self,
        waypoint: &crate::waypoint::Waypoint,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiFollowWaypointPathExactAsTeam()
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowWaypointPathAsTeamExact,
                cmd_source,
            );
            params.waypoint = Some(waypoint.id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_exit_production_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowExitProductionPath,
                cmd_source,
            );
            params.coords = path.to_vec();
            params.obj = ignore_object_id;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_path(
        &self,
        path: &[Coord3D],
        ignore_object_id: Option<ObjectID>,
        cmd_source: CommandSourceType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params =
                crate::ai::AiCommandParams::new(crate::ai::AiCommandType::FollowPath, cmd_source);
            params.coords = path.to_vec();
            params.obj = ignore_object_id;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_follow_path_append(&self, pos: &Coord3D, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowPathAppend,
                cmd_source,
            );
            params.pos = *pos;
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_move_away_from_unit(&self, obj_id: ObjectID, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            if !guard.is_allowed_to_move_away_from_unit() {
                return;
            }
            if let Some(other) = crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
                if let Ok(other_guard) = other.read() {
                    if other_guard.test_status(crate::common::ObjectStatusTypes::IsUsingAbility)
                        || other_guard
                            .get_ai()
                            .and_then(|ai| ai.lock().ok().map(|ai_guard| ai_guard.is_busy()))
                            .unwrap_or(false)
                    {
                        return;
                    }
                }
            }
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveAwayFromUnit,
                cmd_source,
            );
            params.obj = Some(obj_id);
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_guard_retaliate(
        &self,
        victim: &Arc<RwLock<Object>>,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardRetaliate()
        if let Ok(mut guard) = self.try_lock() {
            if let Ok(victim_guard) = victim.read() {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::GuardRetaliate,
                    cmd_source,
                );
                params.obj = Some(victim_guard.get_id());
                params.pos = *pos;
                params.int_value = max_shots_to_fire;
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn ai_guard_position(
        &self,
        pos: &Coord3D,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardPosition()
        // Uses the AI state machine so guard mode and command source are preserved.
        if let Ok(mut guard) = self.try_lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::GuardPosition,
                cmd_source,
            );
            params.pos = *pos;
            params.int_value = guard_mode.as_i32();
            let _ = guard.execute_command(&params);
        }
    }

    fn ai_guard_object(
        &self,
        obj_to_guard: &Arc<RwLock<Object>>,
        guard_mode: GuardMode,
        cmd_source: CommandSourceType,
    ) {
        // C++ Reference: AIUpdateInterface::aiGuardObject()
        if let Ok(mut guard) = self.try_lock() {
            if let Ok(target_guard) = obj_to_guard.read() {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::GuardObject,
                    cmd_source,
                );
                params.obj = Some(target_guard.get_id());
                params.int_value = guard_mode.as_i32();
                let _ = guard.execute_command(&params);
            }
        }
    }

    fn is_idle(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_idle()
        } else {
            false
        }
    }

    fn is_busy(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_busy()
        } else {
            false
        }
    }

    fn set_attitude(&self, attitude: AIAttitudeType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_attitude(attitude);
        }
    }

    fn get_attitude(&self) -> AIAttitudeType {
        if let Ok(guard) = self.try_lock() {
            guard.get_attitude()
        } else {
            AIAttitudeType::Normal
        }
    }

    fn is_ai_in_dead_state(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_ai_in_dead_state()
        } else {
            false
        }
    }

    fn mark_as_dead(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.mark_as_dead();
        }
    }

    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        if let Ok(guard) = self.try_lock() {
            guard.get_goal_object()
        } else {
            None
        }
    }

    fn choose_locomotor_set(&self, set: LocomotorSetType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.choose_locomotor_set(set);
        }
    }

    fn set_allow_invalid_position(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_allow_invalid_position(allow);
        }
    }

    fn set_ultra_accurate(&self, ultra: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_ultra_accurate(ultra);
        }
    }
    fn get_last_command_source(&self) -> CommandSourceType {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_command_source()
        } else {
            CommandSourceType::FromAi
        }
    }

    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        if let Ok(guard) = self.try_lock() {
            guard.get_which_turret_for_cur_weapon()
        } else {
            TurretType::Invalid
        }
    }

    fn set_turret_enabled(&self, turret: TurretType, enabled: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_turret_enabled(turret, enabled);
        }
    }

    fn recenter_turret(&self, turret: TurretType) {
        if let Ok(mut guard) = self.try_lock() {
            guard.recenter_turret(turret);
        }
    }

    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_turret_in_natural_position(turret)
        } else {
            true
        }
    }

    fn get_path(&self) -> Option<()> {
        if let Ok(guard) = self.try_lock() {
            if guard.get_path_destination().is_some() {
                return Some(());
            }
        }
        None
    }

    fn get_path_destination(&self) -> Option<Coord3D> {
        if let Ok(guard) = self.try_lock() {
            guard.get_path_destination()
        } else {
            None
        }
    }

    fn get_locomotor_distance_to_goal(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_locomotor_distance_to_goal()
        } else {
            0.0
        }
    }

    fn get_current_victim(&self) -> Option<ObjectID> {
        if let Ok(guard) = self.try_lock() {
            guard.get_current_victim()
        } else {
            None
        }
    }

    fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_current_victim(victim);
        }
    }

    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        if let Ok(guard) = self.try_lock() {
            guard.get_cur_locomotor()
        } else {
            None
        }
    }

    fn get_preferred_height(&self) -> Option<Real> {
        self.try_lock()
            .ok()
            .and_then(|guard| guard.get_preferred_height())
    }

    fn ai_go_prone(&self, damage_info: &DamageInfo, cmd_source: CommandSourceType) {
        if let Ok(mut guard) = self.try_lock() {
            guard.ai_go_prone(damage_info, cmd_source);
        }
    }

    fn execute_command(
        &self,
        params: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.try_lock() {
            guard.execute_command(params)
        } else {
            Err("Failed to lock AIUpdateInterface".into())
        }
    }

    fn queue_waypoint(&self, pos: &Coord3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.queue_waypoint(pos);
        }
    }

    fn execute_waypoint_queue(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.execute_waypoint_queue();
        }
    }
}

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
        _dock: &Arc<RwLock<Object>>,
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
}

/// Physics behavior interface (matching C++ PhysicsBehavior)
pub trait PhysicsBehavior: Send + Sync + std::fmt::Debug {
    /// Update physics simulation
    fn update(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Get current velocity
    fn get_velocity(&self) -> Vec3D;
    /// Set velocity
    fn set_velocity(&mut self, velocity: &Vec3D);
    /// Check if the object is on ground
    fn is_on_ground(&self) -> bool;

    /// Apply force to the object
    fn apply_force(&mut self, _force: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Set yaw rotation rate (rotation around vertical axis)
    fn set_yaw_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set roll rotation rate (rotation around forward axis)
    fn set_roll_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set pitch rotation rate (rotation around lateral axis)
    fn set_pitch_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set turning state (matches C++ PhysicsBehavior::setTurning).
    fn set_turning(&mut self, _turning: i32) {
        // Default implementation - subclasses should override
    }

    /// Set mass of the physics object
    fn set_mass(&mut self, _mass: Real) {
        // Default implementation - subclasses should override
    }

    /// Set extra friction coefficient
    fn set_extra_friction(&mut self, _friction: Real) {
        // Default implementation - subclasses should override
    }

    /// Set extra bounciness coefficient
    fn set_extra_bounciness(&mut self, _bounciness: Real) {
        // Default implementation - subclasses should override
    }

    /// Enable or disable bouncing
    fn set_allow_bouncing(&mut self, _allow: bool) {
        // Default implementation - subclasses should override
    }

    /// Allow friction while airborne (matches C++ setAllowAirborneFriction).
    fn set_allow_airborne_friction(&mut self, allow: bool) {
        let _ = allow;
    }

    /// Add to current velocity (matches C++ addVelocityTo).
    fn add_velocity_to(&mut self, velocity: &Vec3D) {
        let mut current = self.get_velocity();
        current += *velocity;
        self.set_velocity(&current);
    }

    /// Set rotation angles (yaw, pitch, roll)
    fn set_angles(&mut self, _yaw: Real, _pitch: Real, _roll: Real) {
        // Default implementation - subclasses should override
    }

    /// Get mass of the physics object
    fn get_mass(&self) -> Real {
        // Default implementation - return default mass
        1.0
    }

    /// Set or clear the bounce sound used by collisions.
    fn set_bounce_sound(&mut self, _sound: Option<AudioEventRts>) {}

    /// Get the bounce sound for collision audio.
    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        None
    }

    /// Apply angular velocity (rotational forces)
    fn apply_angular_velocity(&mut self, _angular_velocity: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Apply motive force (propulsion)
    fn apply_motive_force(&mut self, _force: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Get current turning rate
    fn get_turning(&self) -> Real {
        // Default implementation - return zero
        0.0
    }

    /// Apply impulse/shock force (lightweight default).
    fn apply_shock(&mut self, force: &Coord3D) {
        let mass = self.get_mass().max(0.001);
        let impulse = Vec3D::new(force.x / mass, force.y / mass, force.z / mass);
        self.add_velocity_to(&impulse);
    }
    /// Apply a random rotation (lightweight default).
    fn apply_random_rotation(&mut self) {
        let yaw = crate::helpers::get_game_logic_random_value_real(
            -std::f32::consts::PI,
            std::f32::consts::PI,
        );
        let pitch = crate::helpers::get_game_logic_random_value_real(-0.25, 0.25);
        let roll = crate::helpers::get_game_logic_random_value_real(-0.25, 0.25);
        self.set_angles(yaw, pitch, roll);
    }
    /// Toggle stunned state.
    fn set_stunned(&mut self, stunned: bool) {
        let _ = stunned;
    }

    /// Allow object to fall under gravity
    fn set_allow_to_fall(&mut self, allow: bool) {
        let _ = allow;
    }

    /// Whether this object is currently allowed to fall under gravity.
    fn get_allow_to_fall(&self) -> bool {
        true
    }

    /// Clear current acceleration (matches C++ clearAcceleration).
    fn clear_acceleration(&mut self) {}

    /// Scrub horizontal velocity to desired speed (matches C++ scrubVelocity2D).
    fn scrub_velocity_2d(&mut self, desired_velocity: Real) {
        let mut velocity = self.get_velocity();
        if desired_velocity.abs() < 0.001 {
            velocity.x = 0.0;
            velocity.y = 0.0;
            self.set_velocity(&velocity);
            return;
        }
        let cur = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
        if cur <= 0.0 || desired_velocity > cur {
            return;
        }
        let scale = desired_velocity / cur;
        velocity.x *= scale;
        velocity.y *= scale;
        self.set_velocity(&velocity);
    }

    /// Scrub vertical velocity to desired speed (matches C++ scrubVelocityZ).
    fn scrub_velocity_z(&mut self, desired_velocity: Real) {
        let mut velocity = self.get_velocity();
        if desired_velocity.abs() < 0.001 {
            velocity.z = 0.0;
            self.set_velocity(&velocity);
            return;
        }
        if (desired_velocity < 0.0 && velocity.z < desired_velocity)
            || (desired_velocity > 0.0 && velocity.z > desired_velocity)
        {
            velocity.z = desired_velocity;
            self.set_velocity(&velocity);
        }
    }

    /// Reset dynamic physics state (matches C++ PhysicsBehavior::resetDynamicPhysics).
    fn reset_dynamic_physics(&mut self) {
        self.set_velocity(&Vec3D::ZERO);
        self.set_yaw_rate(0.0);
        self.set_pitch_rate(0.0);
        self.set_roll_rate(0.0);
        self.set_angles(0.0, 0.0, 0.0);
    }

    /// Get the ID of the last object this physics object collided with
    fn get_last_collidee(&self) -> ObjectID {
        // Default implementation - return invalid ID (no collision)
        INVALID_ID
    }

    /// Get the ID of the object to ignore collisions with (matches C++ PhysicsBehavior::getIgnoreCollisionsWith).
    fn get_ignore_collisions_with(&self) -> ObjectID {
        INVALID_ID
    }

    /// Ignore collisions with a specific object (matches C++ PhysicsBehavior::setIgnoreCollisionsWith).
    fn set_ignore_collisions_with(&mut self, _obj_id: ObjectID) {
        // Default implementation - subclasses should override if supported
    }
}

/// Extension trait for Arc<Mutex<dyn PhysicsBehavior>> to provide convenient methods
pub trait PhysicsBehaviorExt {
    fn get_velocity(&self) -> Vec3D;
    fn set_velocity(&self, velocity: &Vec3D);
    fn apply_force(&self, force: &Vec3D);
    fn add_velocity_to(&self, velocity: &Vec3D);
    fn set_yaw_rate(&self, rate: Real);
    fn set_roll_rate(&self, rate: Real);
    fn set_pitch_rate(&self, rate: Real);
    fn set_mass(&self, mass: Real);
    fn get_mass(&self) -> Real;
    fn set_extra_friction(&self, friction: Real);
    fn set_extra_bounciness(&self, bounciness: Real);
    fn set_allow_bouncing(&self, allow: bool);
    fn set_allow_airborne_friction(&self, allow: bool);
    fn set_allow_to_fall(&self, allow: bool);
    fn get_allow_to_fall(&self) -> bool;
    fn set_turning(&self, turning: i32);
    fn set_angles(&self, yaw: Real, pitch: Real, roll: Real);
    fn apply_angular_velocity(&self, angular_velocity: &Vec3D);
    fn apply_motive_force(&self, force: &Vec3D);
    fn get_turning(&self) -> Real;
    fn get_last_collidee(&self) -> ObjectID;
    fn set_bounce_sound(&self, sound: Option<AudioEventRts>);
    fn get_bounce_sound(&self) -> Option<AudioEventRts>;
    fn set_ignore_collisions_with(&self, obj_id: ObjectID);
    fn clear_acceleration(&self);
    fn scrub_velocity_2d(&self, desired_velocity: Real);
    fn scrub_velocity_z(&self, desired_velocity: Real);
    fn reset_dynamic_physics(&self);
}

impl PhysicsBehaviorExt for Arc<Mutex<dyn PhysicsBehavior>> {
    fn get_velocity(&self) -> Vec3D {
        if let Ok(guard) = self.try_lock() {
            guard.get_velocity()
        } else {
            Vec3D::ZERO
        }
    }

    fn set_velocity(&self, velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_velocity(velocity);
        }
    }

    fn apply_force(&self, force: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_force(force);
        }
    }

    fn add_velocity_to(&self, velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.add_velocity_to(velocity);
        }
    }

    fn set_yaw_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_yaw_rate(rate);
        }
    }

    fn set_roll_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_roll_rate(rate);
        }
    }

    fn set_pitch_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_pitch_rate(rate);
        }
    }

    fn set_mass(&self, mass: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_mass(mass);
        }
    }

    fn set_extra_friction(&self, friction: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_extra_friction(friction);
        }
    }

    fn set_extra_bounciness(&self, bounciness: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_extra_bounciness(bounciness);
        }
    }

    fn set_allow_bouncing(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_bouncing(allow);
        }
    }

    fn set_allow_airborne_friction(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_airborne_friction(allow);
        }
    }

    fn set_allow_to_fall(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_to_fall(allow);
        }
    }

    fn get_allow_to_fall(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.get_allow_to_fall()
        } else {
            true
        }
    }

    fn set_turning(&self, turning: i32) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_turning(turning);
        }
    }

    fn set_angles(&self, yaw: Real, pitch: Real, roll: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_angles(yaw, pitch, roll);
        }
    }

    fn get_mass(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_mass()
        } else {
            1.0
        }
    }

    fn apply_angular_velocity(&self, angular_velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_angular_velocity(angular_velocity);
        }
    }

    fn apply_motive_force(&self, force: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_motive_force(force);
        }
    }

    fn get_turning(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_turning()
        } else {
            0.0
        }
    }

    fn get_last_collidee(&self) -> ObjectID {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_collidee()
        } else {
            INVALID_ID
        }
    }

    fn set_ignore_collisions_with(&self, obj_id: ObjectID) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_ignore_collisions_with(obj_id);
        }
    }

    fn set_bounce_sound(&self, sound: Option<AudioEventRts>) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_bounce_sound(sound);
        }
    }

    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        if let Ok(guard) = self.try_lock() {
            guard.get_bounce_sound()
        } else {
            None
        }
    }

    fn clear_acceleration(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.clear_acceleration();
        }
    }

    fn scrub_velocity_2d(&self, desired_velocity: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.scrub_velocity_2d(desired_velocity);
        }
    }

    fn scrub_velocity_z(&self, desired_velocity: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.scrub_velocity_z(desired_velocity);
        }
    }

    fn reset_dynamic_physics(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.reset_dynamic_physics();
        }
    }
}

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
        _obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.is_dock_open()
    }

    /// Cancel dock operation for an object
    fn cancel_dock(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Reserve an approach position in the queue
    fn reserve_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Advance to the next approach position
    fn advance_approach_position(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
        approach_pos: &mut i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Check if clear to advance in queue
    fn is_clear_to_advance(
        &self,
        obj: &Arc<RwLock<Object>>,
        approach_position: i32,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Called when approach position reached
    fn on_approach_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Check if clear to enter the dock
    fn is_clear_to_enter(
        &self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Get entry position coordinates
    fn get_enter_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when entry position reached
    fn on_enter_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Get actual dock position coordinates
    fn get_dock_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when dock position reached
    fn on_dock_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Perform dock action (repair, supply, etc.)
    /// Returns true when action is complete
    fn action(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        drone: Option<&Arc<RwLock<Object>>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Get exit position coordinates
    fn get_exit_position(
        &self,
        obj: &Arc<RwLock<Object>>,
        goal_pos: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Called when exit position reached
    fn on_exit_reached(
        &mut self,
        obj: &Arc<RwLock<Object>>,
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

/// Update sleep time type - re-export from object::helper
pub use crate::object::helper::UpdateSleepTime;

/// Convert up to four candidate wake frames into an UpdateSleepTime relative to now.
/// Mirrors C++ UpdateModule::frameToSleepTime behavior.
pub fn frame_to_sleep_time(
    mut frame1: UnsignedInt,
    frame2: UnsignedInt,
    frame3: UnsignedInt,
    frame4: UnsignedInt,
) -> UpdateSleepTime {
    if frame1 > frame2 {
        frame1 = frame2;
    }
    if frame1 > frame3 {
        frame1 = frame3;
    }
    if frame1 > frame4 {
        frame1 = frame4;
    }

    let now = TheGameLogic::get_frame();
    if frame1 > now {
        UpdateSleepTime::frames(frame1 - now)
    } else if frame1 == now {
        UpdateSleepTime::None
    } else {
        log::warn!("frame_to_sleep_time: frame is in the past ({frame1} < {now})");
        UpdateSleepTime::None
    }
}

/// Update module pointer type
pub type UpdateModulePtr = Arc<RwLock<dyn UpdateModuleInterface>>;

/// Minimal no-op update module used in scaffolding and tests.
#[derive(Debug, Default)]
pub struct UpdateModuleDummy;

impl UpdateModuleInterface for UpdateModuleDummy {}

/// Phase ordering for sleepy updates (mirrors C++ SleepyUpdatePhase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SleepyUpdatePhase {
    Initial = 0,
    Physics = 1,
    Normal = 2,
    Final = 3,
}

impl Default for SleepyUpdatePhase {
    fn default() -> Self {
        SleepyUpdatePhase::Normal
    }
}

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
    fn unload_single_object(&mut self, obj: &Arc<RwLock<Object>>);
}

pub trait POWTruckAIUpdateInterface: Send + Sync {
    fn set_task(
        &mut self,
        task: crate::pow_truck_ai_update::POWTruckTask,
        task_object: Option<ObjectID>,
    );
    fn get_current_task(&self) -> crate::pow_truck_ai_update::POWTruckTask;
    fn load_prisoner(&mut self, prisoner: ObjectID);
    fn unload_prisoners_to_prison(&mut self, prison: &Arc<RwLock<Object>>);
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
        _master: &Arc<RwLock<Object>>,
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
        _obj: &Arc<RwLock<crate::object::Object>>,
        _door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Special tunnel-network style exit that preserves the passenger's current AI state.
    fn exit_object_in_a_hurry(
        &mut self,
        _obj: &Arc<RwLock<crate::object::Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        _obj: &Arc<RwLock<crate::object::Object>>,
        _host: Option<&Arc<RwLock<crate::object::Object>>>,
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

/// Extension trait for Arc<Mutex<dyn BodyModuleInterface>> to provide convenient methods
pub trait BodyModuleInterfaceExt {
    fn set_initial_health(&self, health_percent: f32);
    fn get_max_health(&self) -> f32;
    fn get_health(&self) -> f32;
    fn set_health(&self, health: f32);
    fn get_last_damage_info(&self) -> Option<DamageInfo>;
    fn set_max_health(
        &self,
        max_health: f32,
        change_type: crate::object::body::body_module::MaxHealthChangeType,
    );
    fn set_aflame(&self, aflame: bool);
    fn set_damage_state(&self, new_state: BodyDamageType);
    fn attempt_healing(&self, healing_info: &mut DamageInfo);
}

impl BodyModuleInterfaceExt for Arc<Mutex<dyn BodyModuleInterface>> {
    fn set_initial_health(&self, health_percent: f32) {
        if let Ok(mut guard) = self.try_lock() {
            // Convert f32 percent to i32 for the trait method
            let percent_i32 = health_percent.clamp(0.0, 100.0).round() as i32;
            let _ = guard.set_initial_health(percent_i32);
        }
    }

    fn get_max_health(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_max_health()
        } else {
            0.0
        }
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_damage_info()
        } else {
            None
        }
    }

    fn set_max_health(
        &self,
        max_health: f32,
        change_type: crate::object::body::body_module::MaxHealthChangeType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_max_health(max_health, change_type);
        }
    }

    fn get_health(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_health()
        } else {
            0.0
        }
    }

    fn set_health(&self, health: f32) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = BodyModuleInterface::set_health(&mut *guard, health);
        }
    }

    fn set_aflame(&self, aflame: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_aflame(aflame);
        }
    }

    fn set_damage_state(&self, new_state: BodyDamageType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_damage_state(new_state);
        }
    }

    fn attempt_healing(&self, healing_info: &mut DamageInfo) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.attempt_healing(healing_info);
        }
    }
}

/// Extension trait for MutexGuard<dyn BodyModuleInterface> to provide set_health
pub trait BodyModuleGuardExt {
    fn set_health(&mut self, health: f32);
}

impl<'a> BodyModuleGuardExt for std::sync::MutexGuard<'a, dyn BodyModuleInterface> {
    fn set_health(&mut self, health: f32) {
        let _ = BodyModuleInterface::set_health(&mut **self, health);
    }
}

/// Extension trait for Arc<Mutex<dyn BehaviorModuleInterface>> to provide convenient methods
pub trait BehaviorModuleExt {
    fn set_sd_enabled(&self, enabled: bool);
    fn start_fire_spreading(&self);
}

impl BehaviorModuleExt for Arc<Mutex<dyn BehaviorModuleInterface>> {
    fn set_sd_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_sd_enabled(enabled);
        }
    }

    fn start_fire_spreading(&self) {
        // Fire spreading is triggered through FlammableUpdate → FireSpreadUpdate
        // with an UpdateContext (see flammable_update.rs:203). This trait method
        // has no UpdateContext, so the real path handles it.
    }
}

/// Extension trait for Arc<Mutex<ExperienceTracker>> to provide convenient methods
pub trait ExperienceTrackerExt {
    fn set_experience_sink(&self, sink: ObjectID);
}

impl ExperienceTrackerExt for Arc<Mutex<crate::common::ExperienceTracker>> {
    fn set_experience_sink(&self, sink: ObjectID) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_experience_sink(sink);
        }
    }
}

/// Extension trait for Arc<Mutex<StealthController>> to provide convenient methods
pub trait StealthControllerExt {
    fn receive_grant(&self, grant: bool, frames: UnsignedInt, current_frame: UnsignedInt);
}

impl StealthControllerExt for Arc<Mutex<crate::stealth_update::StealthController>> {
    fn receive_grant(&self, grant: bool, frames: UnsignedInt, current_frame: UnsignedInt) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.receive_grant(grant, frames, current_frame);
        }
    }
}

/// Extension trait for Arc<Mutex<dyn SpecialAbilityUpdate>> to provide convenient methods
pub trait SpecialAbilityUpdateExt {
    fn is_active(&self) -> bool;
}

impl SpecialAbilityUpdateExt for Arc<Mutex<dyn SpecialAbilityUpdate>> {
    fn is_active(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_ability_active()
        } else {
            false
        }
    }
}

/// Extension trait for Arc<Mutex<FlammableUpdate>> to provide convenient methods
pub trait FlammableUpdateExt {
    fn try_to_ignite(&self, ctx: &mut crate::common::UpdateContext<'_>);
}

impl FlammableUpdateExt for Arc<Mutex<dyn BehaviorModuleInterface>> {
    fn try_to_ignite(&self, _ctx: &mut crate::common::UpdateContext<'_>) {
        if let Ok(mut guard) = self.lock() {
            guard.try_to_ignite_flammable();
        }
    }
}
