//! Object module - Rust conversion of C++ Object class
//!
//! Simple base object for all game entities. Objects are manipulated via the GameLogic singleton.
//! Author: Michael S. Booth, October 2000 (C++ version)
//! Rust conversion: 2025

pub mod armor;
pub mod behavior;
pub mod body;
pub mod collide;
pub use armor::{Armor, ArmorTemplate, TheArmorStore};
pub mod contain;
pub mod crate_system;
pub mod create;
pub mod damage;
pub mod destroy;
pub mod die;
pub mod draw;
pub mod helper;
pub mod production;
pub mod special_power_cooldown;
pub mod special_power_effects;
pub mod special_power_interface_cast;
pub mod special_power_module;
pub mod special_power_template;
pub mod special_power_types;
pub mod special_powers;
pub mod update;
pub mod upgrade;
pub mod weapon;
// pub mod update_modules;
// pub mod concrete_update_modules;
pub mod drawable;
pub use drawable::{DebrisDrawAnims, DrawableArcExt, apply_debris_draw};
pub mod crate_registry_bind;
pub mod experience_tracker;
pub mod firing_tracker;
pub mod ghost_object;
pub mod iterator;
pub mod locomotor;
pub mod object;
pub mod object_creation_list;
pub mod object_factory;
pub mod object_types;
mod partition_data;
pub mod partition_manager;
pub use partition_data::{
    PartitionData, partition_cell_shroud_status, stamp_partition_cell_lookers,
};

pub mod registry;
pub mod simple_object;
pub mod simple_object_iterator;
pub mod structure;
pub mod types;
#[path = "unit/mod.rs"]
pub mod unit;
pub mod w3d_ghost_object;
pub mod w3d_ghost_object_xfer;
pub mod weapon_set;
pub use crate::common::types::ObjectStatusTypes;
pub use crate::template::ObjectTemplate;
pub use ghost_object::{GhostObject, GhostObjectManager, THE_GHOST_OBJECT_MANAGER};
pub use w3d_ghost_object::{
    FrozenW3DGhostSceneEvent, FrozenW3DGhostSnapshot, THE_W3D_GHOST_OBJECT_MANAGER, W3DGhostObject,
    W3DGhostObjectManager, W3DGhostSnapshotKey, W3DRenderObjectSnapshot,
};

use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock, Weak};

use game_engine::common::thing::module_factory::{
    ModuleFactory, get_module_factory, init_module_factory,
};
use game_engine::common::{
    audio::AudioPriority,
    audio::dynamic_audio_event_info::DynamicAudioEventInfo,
    audio::game_audio::{get_global_audio_manager, initialize_global_audio_manager},
    name_key_generator::NameKeyGenerator,
    system::{Snapshotable as EngineSnapshotable, Xfer as EngineXfer},
    thing::module::{
        self as engine_module, Drawable as ModuleDrawableTrait, Module, ModuleData,
        ModuleInterfaceType, ModuleType, Object as ModuleObjectTrait, Thing as ModuleThing,
        TimeOfDay,
    },
};
use log::warn;

// Forward declarations - assume these exist in other modules
use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
use crate::common::types::ControlBarInterface;
use crate::common::{
    AsciiString, Bool, Byte, Color, CommandSourceType, Coord2D, Coord3D, DefaultThingTemplate,
    Dict, DictType, DisabledMaskType, DisabledType, FormationID, GeometryInfo, ICoord3D, Int,
    KindOf, KindOfMask, KindOfMaskType, LOGICFRAMES_PER_SECOND, Matrix3D, ModelConditionFlags,
    NameKeyType, ObjectID, ObjectShroudStatus, ObjectStatusMaskType, PathfindLayerEnum, PlayerId,
    PlayerMaskType, Real, Relationship, Snapshot, TeamMemberList, Thing, ThingTemplate, TurretType,
    UnsignedByte, UnsignedInt, UpgradeMaskType, VeterancyLevel, WeaponBonusConditionFlags,
};
use game_engine::common::game_common::FOREVER;
use glam::{EulerRot, Mat4};

// Type alias for CommandSource
pub type CommandSource = CommandSourceType;
use crate::ai::HackerAttackMode;
use crate::common::xfer::Xfer;
use crate::contain_module_overrides::ContainModuleDataKind;

use crate::ai::AIGroup;
use crate::attack::{ATTACKRESULT_POSSIBLE, AbleToAttackType, CanAttackResult};
use crate::common::ArmorSetType;
use crate::common::types::WeaponBonusConditionType;
use crate::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::experience::ExperienceTracker;
use crate::helpers::{
    FiringTracker, ObjectDisabledHelper, ObjectHeldHelper, TheGameLogic, ThePartitionManager,
};
use crate::modules::{
    AIAttitudeType, AIUpdateInterface, AIUpdateInterfaceExt, BehaviorModuleInterface,
    BodyModuleInterface, BodyModuleInterfaceExt, CollideModuleInterface, ContainModuleInterface,
    CountermeasuresBehaviorInterface, CreateModuleInterface, DamageModule, DestroyModuleInterface,
    DieModuleInterface, DockUpdateInterface, ExitInterface, PhysicsBehavior,
    PowerPlantUpdateInterface, ProductionUpdateInterface, ProjectileUpdateInterface,
    RailedTransportDockUpdateInterface, SlavedUpdateInterface, SleepyUpdatePhase,
    SpawnBehaviorInterface, SpawnBehaviorInterfaceExt, SpecialAbilityUpdate,
    SpecialPowerModuleInterface, SpecialPowerModuleInterfaceExt, SpecialPowerUpdateInterface,
    UpdateModule, UpdateModuleInterface, UpdateModulePtr, UpdateSleepTime, UpgradeModuleInterface,
};
use crate::object::behavior::flight_deck_behavior::FlightDeckBehaviorModule;
use crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehaviorModule;
use crate::object::behavior::special_ability_update::SpecialAbilityUpdate as SpecialAbilityUpdateBehavior;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::die::DieModuleWrapper;
use crate::object::drawable::{Drawable, DrawableExt, DrawableModuleHandle, DrawableThingHandle};
use crate::object::helper::{
    ObjectDefectionHelper, ObjectDefectionHelperModuleData, ObjectHelperInterface,
    ObjectRepulsorHelper, ObjectRepulsorHelperModuleData, ObjectSMCHelper,
    ObjectSMCHelperModuleData, ObjectWeaponStatusHelper, StatusDamageHelper, SubdualDamageHelper,
    TempWeaponBonusHelper,
};
use crate::object::registry::OBJECT_REGISTRY;

/// Wave 264: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

use crate::GameLogicResult;
use crate::object::special_power_types::{SpecialPowerMask, SpecialPowerType};
use crate::object::upgrade::passengers_fire_upgrade::PassengersFireUpgradeHandle;
use crate::object::upgrade::status_bits_upgrade::StatusBitsUpgradeHandle;
use crate::object::upgrade::subobjects_upgrade::SubObjectsUpgradeHandle;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::player::{Player, PlayerIndex, PlayerType, player_list};
use crate::scripting::engine::get_event_manager;
use crate::scripting::events::{GameEvent, GameEventType};
use crate::scripting::{ScriptPriority, ScriptValue};
use crate::stealth_update::StealthUpdateHandle;
use crate::team::{Team, TeamID};
use crate::upgrade::UpgradeTemplate;
use crate::upgrade::center::get_upgrade_center;
use crate::upgrade_legacy::upgrade_mask_for_ascii;
use crate::weapon::{
    Weapon, WeaponAntiMask, WeaponBonusConditionType as WeaponModuleBonusConditionType,
    WeaponChoiceCriteria, WeaponLockType, WeaponSet, WeaponSetFlags, WeaponSetType, WeaponSlotType,
    WeaponStatus,
};

pub trait ObjectLockExt {
    fn lock(&self) -> std::sync::LockResult<std::sync::RwLockWriteGuard<'_, Object>>;
    fn try_lock(&self) -> std::sync::TryLockResult<std::sync::RwLockWriteGuard<'_, Object>>;
}

struct SpecialAbilityUpdateProxy {
    behavior: Arc<Mutex<dyn BehaviorModuleInterface>>,
}

#[allow(dead_code)]
struct ModuleSpecialAbilityUpdateProxy {
    entry: Arc<ModuleEntry>,
}

struct ExitInterfaceProxy {
    behavior: Arc<Mutex<dyn BehaviorModuleInterface>>,
}

struct ContainExitInterfaceProxy {
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
}

struct ModuleExitInterfaceProxy {
    entry: Arc<ModuleEntry>,
}

enum ProductionBehaviorModuleKindMut<'a> {
    QueueExit(&'a mut QueueProductionExitBehaviorModule),
    DefaultExit(
        &'a mut crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule,
    ),
    SpawnPointExit(
        &'a mut crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule,
    ),
    SupplyCenterExit(
        &'a mut crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule,
    ),
    ParkingPlace(
        &'a mut crate::object::behavior::parking_place_behavior::ParkingPlaceBehaviorModule,
    ),
    FlightDeck(&'a mut FlightDeckBehaviorModule),
}

impl<'a> ProductionBehaviorModuleKindMut<'a> {
    fn is_exit_capable(&self) -> bool {
        matches!(
            self,
            Self::QueueExit(_)
                | Self::DefaultExit(_)
                | Self::SpawnPointExit(_)
                | Self::SupplyCenterExit(_)
                | Self::FlightDeck(_)
        )
    }

    fn into_exit_interface(self) -> Option<&'a mut dyn ExitInterface> {
        match self {
            Self::QueueExit(module) => Some(module.behavior_mut()),
            Self::DefaultExit(module) => Some(module.behavior_mut()),
            Self::SpawnPointExit(module) => Some(module.behavior_mut()),
            Self::SupplyCenterExit(module) => Some(module.behavior_mut()),
            Self::FlightDeck(module) => Some(module.behavior_mut()),
            Self::ParkingPlace(_) => None,
        }
    }

    fn into_parking_place_interface(
        self,
    ) -> Option<&'a mut dyn crate::object::behavior::behavior_module::ParkingPlaceBehaviorInterface>
    {
        match self {
            Self::ParkingPlace(module) => Some(module.behavior_mut()),
            Self::FlightDeck(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn into_flight_deck_behavior(
        self,
    ) -> Option<&'a mut crate::object::behavior::flight_deck_behavior::FlightDeckBehavior> {
        match self {
            Self::FlightDeck(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn set_rally_point(self, pos: &Coord3D) -> bool {
        match self {
            Self::QueueExit(module) => {
                module.behavior_mut().set_rally_point(*pos);
                true
            }
            Self::DefaultExit(module) => {
                module.behavior_mut().set_rally_point(*pos);
                true
            }
            Self::SupplyCenterExit(module) => {
                module.behavior_mut().set_rally_point(*pos);
                true
            }
            Self::ParkingPlace(module) => {
                module.behavior_mut().set_rally_point(pos);
                true
            }
            Self::FlightDeck(module) => {
                module.behavior_mut().set_rally_point(Some(*pos));
                true
            }
            Self::SpawnPointExit(_) => false,
        }
    }
}

fn module_production_behavior_kind(
    module: &mut dyn Module,
) -> Option<ProductionBehaviorModuleKindMut<'_>> {
    if module.as_any().is::<QueueProductionExitBehaviorModule>() {
        return (module as &mut dyn Any)
            .downcast_mut::<QueueProductionExitBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::QueueExit(m));
    }
    if module.as_any().is::<crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule>() {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::DefaultExit(m));
    }
    if module.as_any().is::<crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule>() {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::SpawnPointExit(m));
    }
    if module.as_any().is::<crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule>() {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::SupplyCenterExit(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::parking_place_behavior::ParkingPlaceBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::parking_place_behavior::ParkingPlaceBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::ParkingPlace(m));
    }
    if module.as_any().is::<FlightDeckBehaviorModule>() {
        return (module as &mut dyn Any)
            .downcast_mut::<FlightDeckBehaviorModule>()
            .map(|m| ProductionBehaviorModuleKindMut::FlightDeck(m));
    }

    None
}

enum DockUpdateModuleKindMut<'a> {
    RepairDock(&'a mut crate::object::production::dock_update::RepairDockUpdateModule),
    SupplyCenterDock(&'a mut crate::object::production::dock_update::SupplyCenterDockUpdateModule),
    SupplyWarehouseDock(
        &'a mut crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdateModule,
    ),
    #[cfg(feature = "allow_surrender")]
    PrisonDock(&'a mut crate::object::production::prison_dock::PrisonDockUpdateModule),
    RailedTransportDock(
        &'a mut crate::object::production::railed_transport_dock::RailedTransportDockUpdateModule,
    ),
}

impl<'a> DockUpdateModuleKindMut<'a> {
    fn into_dock_interface(self) -> &'a mut dyn DockUpdateInterface {
        match self {
            Self::RepairDock(module) => module.behavior_mut(),
            Self::SupplyCenterDock(module) => module.behavior_mut(),
            Self::SupplyWarehouseDock(module) => module.behavior_mut(),
            #[cfg(feature = "allow_surrender")]
            Self::PrisonDock(module) => module.behavior_mut(),
            Self::RailedTransportDock(module) => module.behavior_mut(),
        }
    }

    fn into_railed_transport_interface(
        self,
    ) -> Option<&'a mut dyn RailedTransportDockUpdateInterface> {
        match self {
            Self::RailedTransportDock(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }
}

fn module_dock_update_kind(module: &mut dyn Module) -> Option<DockUpdateModuleKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::production::dock_update::RepairDockUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::dock_update::RepairDockUpdateModule>()
            .map(|m| DockUpdateModuleKindMut::RepairDock(m));
    }
    if module
        .as_any()
        .is::<crate::object::production::dock_update::SupplyCenterDockUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::dock_update::SupplyCenterDockUpdateModule>()
            .map(|m| DockUpdateModuleKindMut::SupplyCenterDock(m));
    }
    if module
        .as_any()
        .is::<crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdateModule>()
            .map(|m| DockUpdateModuleKindMut::SupplyWarehouseDock(m));
    }
    #[cfg(feature = "allow_surrender")]
    if module
        .as_any()
        .is::<crate::object::production::prison_dock::PrisonDockUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::prison_dock::PrisonDockUpdateModule>()
            .map(|m| DockUpdateModuleKindMut::PrisonDock(m));
    }
    if module
        .as_any()
        .is::<crate::object::production::railed_transport_dock::RailedTransportDockUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::railed_transport_dock::RailedTransportDockUpdateModule>()
            .map(|m| DockUpdateModuleKindMut::RailedTransportDock(m));
    }

    None
}

enum ProductionQueueModuleKindMut<'a> {
    Complete(&'a mut crate::object::production::production_update_complete::ProductionUpdateCompleteModule),
}

impl<'a> ProductionQueueModuleKindMut<'a> {
    fn request_unique_unit_id(self) -> Option<u32> {
        match self {
            Self::Complete(module) => Some(module.behavior_mut().request_unique_unit_id()),
        }
    }

    fn queue_unit(
        self,
        template_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> bool {
        match self {
            Self::Complete(module) => module
                .behavior_mut()
                .queue_create_unit(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                )
                .is_ok(),
        }
    }

    fn queue_unit_with_production_id(
        self,
        template_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
        production_id: u32,
    ) -> bool {
        match self {
            Self::Complete(module) => module
                .behavior_mut()
                .queue_create_unit_with_id(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                    production_id,
                )
                .is_ok(),
        }
    }

    fn queue_upgrade(
        self,
        upgrade_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> bool {
        match self {
            Self::Complete(module) => {
                if module.behavior().has_any_upgrade_in_queue() {
                    return false;
                }
                module
                    .behavior_mut()
                    .queue_upgrade(upgrade_name, build_cost, build_time, player_id)
                    .is_ok()
            }
        }
    }

    fn cancel_upgrade(self, upgrade_name: &str) -> bool {
        match self {
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .behavior_mut()
                    .cancel_upgrade_by_name(upgrade_name, &mut refund)
                    .is_ok()
            }
        }
    }

    fn cancel_unit_by_template_name(self, template_name: &str) -> bool {
        match self {
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .behavior_mut()
                    .cancel_unit_by_template_name(template_name, &mut refund)
                    .is_ok()
            }
        }
    }

    fn cancel_unit_by_production_id(self, production_id: u32) -> bool {
        match self {
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .behavior_mut()
                    .cancel_unit_by_production_id(production_id, &mut refund)
                    .is_ok()
            }
        }
    }

    fn set_enabled(self, enabled: bool) {
        match self {
            Self::Complete(module) => {
                if enabled {
                    module.behavior_mut().resume_production();
                } else {
                    module.behavior_mut().pause_production();
                }
            }
        }
    }

    fn cancel_and_refund_all(self) {
        match self {
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .behavior_mut()
                    .cancel_and_refund_all_production(&mut refund);
            }
        }
    }
}

fn module_production_queue_kind(
    module: &mut dyn Module,
) -> Option<ProductionQueueModuleKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::production::production_update_complete::ProductionUpdateCompleteModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::production_update_complete::ProductionUpdateCompleteModule>()
            .map(|m| ProductionQueueModuleKindMut::Complete(m));
    }

    None
}

enum ProductionBehaviorQueueKindMut<'a> {
    Legacy(&'a mut crate::object::behavior::production_update_behavior::ProductionUpdateBehavior),
    Complete(&'a mut crate::object::production::ProductionUpdateComplete),
    Core(&'a mut crate::object::production::ProductionUpdate),
}

impl<'a> ProductionBehaviorQueueKindMut<'a> {
    fn request_unique_unit_id(self) -> Option<u32> {
        match self {
            Self::Legacy(module) => Some(module.request_unique_unit_id()),
            Self::Complete(module) => Some(module.request_unique_unit_id()),
            Self::Core(_) => None,
        }
    }

    fn queue_unit(
        self,
        template_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> bool {
        match self {
            Self::Legacy(module) => {
                let production_id = module.request_unique_unit_id();
                module
                    .queue_create_unit(template_name, production_id)
                    .is_ok()
            }
            Self::Complete(module) => module
                .queue_create_unit(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                )
                .is_ok(),
            Self::Core(module) => module
                .enqueue_production(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                )
                .is_ok(),
        }
    }

    fn queue_unit_with_production_id(
        self,
        template_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
        production_id: u32,
    ) -> bool {
        match self {
            Self::Legacy(module) => module
                .queue_create_unit(template_name, production_id)
                .is_ok(),
            Self::Complete(module) => module
                .queue_create_unit_with_id(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                    production_id,
                )
                .is_ok(),
            Self::Core(module) => module
                .enqueue_production_with_id(
                    template_name,
                    crate::object::production::ProductionType::Unit,
                    build_cost,
                    build_time,
                    player_id,
                    production_id,
                )
                .is_ok(),
        }
    }

    fn queue_upgrade(
        self,
        upgrade_name: String,
        build_cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> bool {
        match self {
            Self::Legacy(module) => {
                if module.has_any_upgrade_in_queue() {
                    return false;
                }
                module.queue_upgrade(upgrade_name).is_ok()
            }
            Self::Complete(module) => {
                if module.has_any_upgrade_in_queue() {
                    return false;
                }
                module
                    .queue_upgrade(upgrade_name, build_cost, build_time, player_id)
                    .is_ok()
            }
            Self::Core(module) => {
                if module.has_any_upgrade_in_queue() {
                    return false;
                }
                module
                    .enqueue_production(
                        upgrade_name,
                        crate::object::production::ProductionType::Upgrade,
                        build_cost,
                        build_time,
                        player_id,
                    )
                    .is_ok()
            }
        }
    }

    fn cancel_upgrade(self, upgrade_name: &str) -> bool {
        match self {
            Self::Legacy(module) => module.cancel_upgrade(upgrade_name).is_some(),
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .cancel_upgrade_by_name(upgrade_name, &mut refund)
                    .is_ok()
            }
            Self::Core(module) => module.cancel_upgrade_by_name(upgrade_name).is_ok(),
        }
    }

    fn cancel_unit_by_template_name(self, template_name: &str) -> bool {
        match self {
            Self::Legacy(module) => module.cancel_one_unit_of_type(template_name).is_some(),
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .cancel_unit_by_template_name(template_name, &mut refund)
                    .is_ok()
            }
            Self::Core(module) => module.cancel_unit_by_template_name(template_name).is_ok(),
        }
    }

    fn cancel_unit_by_production_id(self, production_id: u32) -> bool {
        match self {
            Self::Legacy(module) => module.cancel_unit_create(production_id).is_some(),
            Self::Complete(module) => {
                let mut refund = |player_id: ObjectID, credits: i32| {
                    if credits <= 0 {
                        return;
                    }
                    if let Ok(list) = player_list().read() {
                        if let Some(player_arc) = list.get_player(player_id as i32) {
                            if let Ok(mut player) = player_arc.write() {
                                player.get_money_mut().add_money(credits);
                            }
                        }
                    }
                };
                module
                    .cancel_unit_by_production_id(production_id, &mut refund)
                    .is_ok()
            }
            Self::Core(module) => module.cancel_unit_by_production_id(production_id).is_ok(),
        }
    }

    fn apply_production_enabled(self, enabled: bool) -> bool {
        match self {
            Self::Legacy(_) => false,
            Self::Complete(module) => {
                if enabled {
                    module.resume_production();
                } else {
                    module.pause_production();
                }
                true
            }
            Self::Core(module) => {
                module.set_production_enabled(enabled);
                true
            }
        }
    }
}

fn behavior_production_queue_kind(
    behavior: &mut dyn BehaviorModuleInterface,
) -> Option<ProductionBehaviorQueueKindMut<'_>> {
    if behavior
        .as_any()
        .is::<crate::object::behavior::production_update_behavior::ProductionUpdateBehavior>()
    {
        return behavior_downcast_mut::<
            crate::object::behavior::production_update_behavior::ProductionUpdateBehavior,
        >(behavior)
        .map(|b| ProductionBehaviorQueueKindMut::Legacy(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::production::ProductionUpdateComplete>()
    {
        return behavior_downcast_mut::<crate::object::production::ProductionUpdateComplete>(
            behavior,
        )
        .map(|b| ProductionBehaviorQueueKindMut::Complete(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::production::ProductionUpdate>()
    {
        return behavior_downcast_mut::<crate::object::production::ProductionUpdate>(behavior)
            .map(|b| ProductionBehaviorQueueKindMut::Core(b));
    }

    None
}

enum ProductionBehaviorRallyKindMut<'a> {
    QueueExit(&'a mut crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehavior),
    DefaultExit(
        &'a mut crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehavior,
    ),
    SupplyCenterExit(
        &'a mut crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehavior,
    ),
    ParkingPlace(&'a mut crate::object::behavior::parking_place_behavior::ParkingPlaceBehavior),
    FlightDeck(&'a mut crate::object::behavior::flight_deck_behavior::FlightDeckBehavior),
}

impl<'a> ProductionBehaviorRallyKindMut<'a> {
    fn set_rally_point(self, pos: &Coord3D) {
        match self {
            Self::QueueExit(module) => module.set_rally_point(*pos),
            Self::DefaultExit(module) => module.set_rally_point(*pos),
            Self::SupplyCenterExit(module) => module.set_rally_point(*pos),
            Self::ParkingPlace(module) => module.set_rally_point(pos),
            Self::FlightDeck(module) => module.set_rally_point(Some(*pos)),
        }
    }

    fn into_flight_deck(
        self,
    ) -> Option<&'a mut crate::object::behavior::flight_deck_behavior::FlightDeckBehavior> {
        match self {
            Self::FlightDeck(module) => Some(module),
            _ => None,
        }
    }
}

fn behavior_production_rally_kind(
    behavior: &mut dyn BehaviorModuleInterface,
) -> Option<ProductionBehaviorRallyKindMut<'_>> {
    if behavior
        .as_any()
        .is::<crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehavior>()
    {
        return behavior_downcast_mut::<crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehavior>(behavior)
            .map(|b| ProductionBehaviorRallyKindMut::QueueExit(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehavior>()
    {
        return behavior_downcast_mut::<crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehavior>(behavior)
            .map(|b| ProductionBehaviorRallyKindMut::DefaultExit(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehavior>()
    {
        return behavior_downcast_mut::<crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehavior>(behavior)
            .map(|b| ProductionBehaviorRallyKindMut::SupplyCenterExit(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::behavior::parking_place_behavior::ParkingPlaceBehavior>()
    {
        return behavior_downcast_mut::<
            crate::object::behavior::parking_place_behavior::ParkingPlaceBehavior,
        >(behavior)
        .map(|b| ProductionBehaviorRallyKindMut::ParkingPlace(b));
    }
    if behavior
        .as_any()
        .is::<crate::object::behavior::flight_deck_behavior::FlightDeckBehavior>()
    {
        return behavior_downcast_mut::<
            crate::object::behavior::flight_deck_behavior::FlightDeckBehavior,
        >(behavior)
        .map(|b| ProductionBehaviorRallyKindMut::FlightDeck(b));
    }

    None
}

enum BehaviorUtilityModuleKindMut<'a> {
    FiringTracker(
        &'a mut crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule,
    ),
    HordeUpdate(&'a mut crate::object::behavior::horde_update::HordeUpdateModule),
    SpawnBehavior(&'a mut crate::object::behavior::spawn_behavior::SpawnBehaviorModule),
    SlavedUpdate(&'a mut crate::object::update::slaved_update::SlavedUpdateModule),
    PowerPlantUpdate(&'a mut crate::object::behavior::power_plant_update::PowerPlantUpdateModule),
    Overcharge(&'a mut crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule),
    TechBuilding(
        &'a mut crate::object::behavior::tech_building_behavior::TechBuildingBehaviorModule,
    ),
    PropagandaTower(
        &'a mut crate::object::behavior::propaganda_tower_behavior::PropagandaTowerBehaviorModule,
    ),
}

impl<'a> BehaviorUtilityModuleKindMut<'a> {
    fn into_firing_tracker(
        self,
    ) -> Option<&'a mut crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule>
    {
        match self {
            Self::FiringTracker(module) => Some(module),
            _ => None,
        }
    }

    fn into_horde_interface(self) -> Option<&'a mut dyn crate::modules::HordeUpdateInterface> {
        match self {
            Self::HordeUpdate(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn into_spawn_interface(
        self,
    ) -> Option<&'a mut dyn crate::object::behavior::spawn_behavior::SpawnBehaviorInterface> {
        match self {
            Self::SpawnBehavior(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn into_slaved_update_interface(self) -> Option<&'a mut dyn SlavedUpdateInterface> {
        match self {
            Self::SlavedUpdate(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn into_power_plant_update_interface(self) -> Option<&'a mut dyn PowerPlantUpdateInterface> {
        match self {
            Self::PowerPlantUpdate(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn overcharge_active(self) -> Option<bool> {
        match self {
            Self::Overcharge(module) => Some(module.behavior().is_overcharge_active()),
            _ => None,
        }
    }

    fn into_overcharge_interface(
        self,
    ) -> Option<&'a mut dyn crate::object::behavior::behavior_module::OverchargeBehaviorInterface>
    {
        match self {
            Self::Overcharge(module) => Some(module.behavior_mut()),
            _ => None,
        }
    }

    fn notify_capture(
        self,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
        match self {
            Self::Overcharge(module) => module.behavior_mut().on_capture(old_owner, new_owner),
            Self::TechBuilding(module) => {
                let _ = module.behavior_mut().on_capture(None, None);
            }
            Self::PropagandaTower(module) => module.behavior_mut().on_capture(old_owner, new_owner),
            Self::FiringTracker(_)
            | Self::HordeUpdate(_)
            | Self::SpawnBehavior(_)
            | Self::SlavedUpdate(_)
            | Self::PowerPlantUpdate(_) => {}
        }
    }
}

fn module_behavior_utility_kind(
    module: &mut dyn Module,
) -> Option<BehaviorUtilityModuleKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule>()
            .map(|m| BehaviorUtilityModuleKindMut::FiringTracker(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::horde_update::HordeUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::horde_update::HordeUpdateModule>()
            .map(|m| BehaviorUtilityModuleKindMut::HordeUpdate(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::spawn_behavior::SpawnBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::spawn_behavior::SpawnBehaviorModule>()
            .map(|m| BehaviorUtilityModuleKindMut::SpawnBehavior(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::power_plant_update::PowerPlantUpdateModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::power_plant_update::PowerPlantUpdateModule>()
            .map(|m| BehaviorUtilityModuleKindMut::PowerPlantUpdate(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule>(
            )
            .map(|m| BehaviorUtilityModuleKindMut::Overcharge(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::tech_building_behavior::TechBuildingBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::tech_building_behavior::TechBuildingBehaviorModule>()
            .map(|m| BehaviorUtilityModuleKindMut::TechBuilding(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::propaganda_tower_behavior::PropagandaTowerBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::propaganda_tower_behavior::PropagandaTowerBehaviorModule>()
            .map(|m| BehaviorUtilityModuleKindMut::PropagandaTower(m));
    }
    if let Some(module) = (module as &mut dyn Any)
        .downcast_mut::<crate::object::update::slaved_update::SlavedUpdateModule>(
    ) {
        return Some(BehaviorUtilityModuleKindMut::SlavedUpdate(module));
    }

    None
}

enum UpgradeModuleKindMut<'a> {
    StatusBits(&'a mut crate::object::upgrade::status_bits_upgrade::StatusBitsUpgrade),
    PassengersFire(&'a mut crate::object::upgrade::passengers_fire_upgrade::PassengersFireUpgrade),
    SubObjects(&'a mut crate::object::upgrade::subobjects_upgrade::SubObjectsUpgrade),
    GrantScience(&'a mut crate::object::upgrade::grant_science_upgrade::GrantScienceUpgrade),
    CommandSet(&'a mut crate::object::upgrade::command_set_upgrade::CommandSetUpgrade),
    WeaponSet(&'a mut crate::object::upgrade::weapon_set_upgrade::WeaponSetUpgrade),
    Radar(&'a mut crate::object::upgrade::radar_upgrade::RadarUpgrade),
    PowerPlant(&'a mut crate::object::upgrade::power_plant_upgrade::PowerPlantUpgrade),
    WeaponBonus(&'a mut crate::object::upgrade::weapon_bonus_upgrade::WeaponBonusUpgrade),
    Stealth(&'a mut crate::object::upgrade::stealth_upgrade::StealthUpgrade),
    ModelCondition(&'a mut crate::object::upgrade::model_condition_upgrade::ModelConditionUpgrade),
    Armor(&'a mut crate::object::upgrade::armor_upgrade::ArmorUpgrade),
    CostModifier(&'a mut crate::object::upgrade::cost_modifier_upgrade::CostModifierUpgrade),
    LocomotorSet(&'a mut crate::object::upgrade::locomotor_set_upgrade::LocomotorSetUpgrade),
    ExperienceScalar(
        &'a mut crate::object::upgrade::experience_scalar_upgrade::ExperienceScalarUpgrade,
    ),
    MaxHealth(&'a mut crate::object::upgrade::max_health_upgrade::MaxHealthUpgrade),
    ActiveShroud(&'a mut crate::object::upgrade::active_shroud_upgrade::ActiveShroudUpgrade),
    ReplaceObject(&'a mut crate::object::upgrade::replace_object_upgrade::ReplaceObjectUpgrade),
    UnpauseSpecialPower(
        &'a mut crate::object::upgrade::unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade,
    ),
    ObjectCreation(&'a mut crate::object::upgrade::object_creation_upgrade::ObjectCreationUpgrade),
    AutoHeal(&'a mut crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule),
}

impl<'a> UpgradeModuleKindMut<'a> {
    fn into_interface(self) -> &'a mut dyn UpgradeModuleInterface {
        match self {
            Self::StatusBits(module) => module,
            Self::PassengersFire(module) => module,
            Self::SubObjects(module) => module,
            Self::GrantScience(module) => module,
            Self::CommandSet(module) => module,
            Self::WeaponSet(module) => module,
            Self::Radar(module) => module,
            Self::PowerPlant(module) => module,
            Self::WeaponBonus(module) => module,
            Self::Stealth(module) => module,
            Self::ModelCondition(module) => module,
            Self::Armor(module) => module,
            Self::CostModifier(module) => module,
            Self::LocomotorSet(module) => module,
            Self::ExperienceScalar(module) => module,
            Self::MaxHealth(module) => module,
            Self::ActiveShroud(module) => module,
            Self::ReplaceObject(module) => module,
            Self::UnpauseSpecialPower(module) => module,
            Self::ObjectCreation(module) => module,
            Self::AutoHeal(module) => module.behavior_mut(),
        }
    }
}

fn module_upgrade_kind(module: &mut dyn Module) -> Option<UpgradeModuleKindMut<'_>> {
    if module
        .as_any()
        .is::<crate::object::upgrade::status_bits_upgrade::StatusBitsUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::status_bits_upgrade::StatusBitsUpgrade>()
            .map(|m| UpgradeModuleKindMut::StatusBits(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::passengers_fire_upgrade::PassengersFireUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::passengers_fire_upgrade::PassengersFireUpgrade>(
            )
            .map(|m| UpgradeModuleKindMut::PassengersFire(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::subobjects_upgrade::SubObjectsUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::subobjects_upgrade::SubObjectsUpgrade>()
            .map(|m| UpgradeModuleKindMut::SubObjects(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::grant_science_upgrade::GrantScienceUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::grant_science_upgrade::GrantScienceUpgrade>()
            .map(|m| UpgradeModuleKindMut::GrantScience(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::command_set_upgrade::CommandSetUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::command_set_upgrade::CommandSetUpgrade>()
            .map(|m| UpgradeModuleKindMut::CommandSet(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::weapon_set_upgrade::WeaponSetUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::weapon_set_upgrade::WeaponSetUpgrade>()
            .map(|m| UpgradeModuleKindMut::WeaponSet(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
            .map(|m| UpgradeModuleKindMut::Radar(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::power_plant_upgrade::PowerPlantUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::power_plant_upgrade::PowerPlantUpgrade>()
            .map(|m| UpgradeModuleKindMut::PowerPlant(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::weapon_bonus_upgrade::WeaponBonusUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::weapon_bonus_upgrade::WeaponBonusUpgrade>()
            .map(|m| UpgradeModuleKindMut::WeaponBonus(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::stealth_upgrade::StealthUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::stealth_upgrade::StealthUpgrade>()
            .map(|m| UpgradeModuleKindMut::Stealth(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::model_condition_upgrade::ModelConditionUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::model_condition_upgrade::ModelConditionUpgrade>(
            )
            .map(|m| UpgradeModuleKindMut::ModelCondition(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::armor_upgrade::ArmorUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::armor_upgrade::ArmorUpgrade>()
            .map(|m| UpgradeModuleKindMut::Armor(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::cost_modifier_upgrade::CostModifierUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::cost_modifier_upgrade::CostModifierUpgrade>()
            .map(|m| UpgradeModuleKindMut::CostModifier(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::locomotor_set_upgrade::LocomotorSetUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::locomotor_set_upgrade::LocomotorSetUpgrade>()
            .map(|m| UpgradeModuleKindMut::LocomotorSet(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::experience_scalar_upgrade::ExperienceScalarUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::experience_scalar_upgrade::ExperienceScalarUpgrade>()
            .map(|m| UpgradeModuleKindMut::ExperienceScalar(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::max_health_upgrade::MaxHealthUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::max_health_upgrade::MaxHealthUpgrade>()
            .map(|m| UpgradeModuleKindMut::MaxHealth(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::active_shroud_upgrade::ActiveShroudUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::active_shroud_upgrade::ActiveShroudUpgrade>()
            .map(|m| UpgradeModuleKindMut::ActiveShroud(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::replace_object_upgrade::ReplaceObjectUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::replace_object_upgrade::ReplaceObjectUpgrade>()
            .map(|m| UpgradeModuleKindMut::ReplaceObject(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::unpause_special_power_upgrade::UnpauseSpecialPowerUpgrade>()
            .map(|m| UpgradeModuleKindMut::UnpauseSpecialPower(m));
    }
    if module
        .as_any()
        .is::<crate::object::upgrade::object_creation_upgrade::ObjectCreationUpgrade>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::upgrade::object_creation_upgrade::ObjectCreationUpgrade>(
            )
            .map(|m| UpgradeModuleKindMut::ObjectCreation(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule>()
            .map(|m| UpgradeModuleKindMut::AutoHeal(m));
    }

    None
}

enum DieModuleKindMut<'a> {
    Wrapper(&'a mut DieModuleWrapper),
    LegacyBox(&'a mut Box<dyn DieModuleInterface>),
    Minefield(&'a mut crate::object::behavior::minefield_behavior::MinefieldBehaviorModule),
    ProductionUpdate(
        &'a mut crate::object::production::production_update_complete::ProductionUpdateCompleteModule,
    ),
    SlowDeath(&'a mut crate::object::behavior::slow_death_behavior::SlowDeathBehavior),
    Bridge(&'a mut crate::object::behavior::bridge_behavior::BridgeBehaviorModule),
    BridgeTower(&'a mut crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule),
}

impl<'a> DieModuleKindMut<'a> {
    fn into_interface(self) -> &'a mut dyn DieModuleInterface {
        match self {
            Self::Wrapper(module) => module,
            Self::LegacyBox(module) => module.as_mut(),
            Self::Minefield(module) => module.behavior_mut(),
            Self::ProductionUpdate(module) => module.behavior_mut(),
            Self::SlowDeath(module) => module,
            Self::Bridge(module) => module.behavior_mut(),
            Self::BridgeTower(module) => module.behavior_mut(),
        }
    }
}

fn module_die_kind(module: &mut dyn Module) -> Option<DieModuleKindMut<'_>> {
    if module.as_any().is::<DieModuleWrapper>() {
        return (module as &mut dyn Any)
            .downcast_mut::<DieModuleWrapper>()
            .map(|m| DieModuleKindMut::Wrapper(m));
    }
    if module
        .as_any()
        .is::<crate::object::behavior::minefield_behavior::MinefieldBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::minefield_behavior::MinefieldBehaviorModule>()
            .map(DieModuleKindMut::Minefield);
    }
    if module.as_any().is::<
        crate::object::production::production_update_complete::ProductionUpdateCompleteModule,
    >() {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::production::production_update_complete::ProductionUpdateCompleteModule>()
            .map(DieModuleKindMut::ProductionUpdate);
    }
    if module
        .as_any()
        .is::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>()
            .map(DieModuleKindMut::SlowDeath);
    }
    if module.as_any().is::<Box<dyn DieModuleInterface>>() {
        return (module as &mut dyn Any)
            .downcast_mut::<Box<dyn DieModuleInterface>>()
            .map(DieModuleKindMut::LegacyBox);
    }
    if module
        .as_any()
        .is::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>()
            .map(DieModuleKindMut::Bridge);
    }
    if module
        .as_any()
        .is::<crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule>()
    {
        return (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule>()
            .map(DieModuleKindMut::BridgeTower);
    }

    None
}

impl SpecialAbilityUpdate for SpecialAbilityUpdateProxy {
    fn update_ability(
        &mut self,
        frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(update) = guard.get_special_power_update_interface() {
                return update.update_special_power(frame_time);
            }
        }
        Ok(())
    }

    fn is_ability_active(&self) -> bool {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(update) = guard.get_special_power_update_interface() {
                return update.is_active();
            }
        }
        false
    }
}

impl SpecialAbilityUpdate for ModuleSpecialAbilityUpdateProxy {
    fn update_ability(
        &mut self,
        frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut result = Ok(());
        self.entry.with_module(|module| {
            if let Some(update) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule>()
            {
                result = update.behavior_mut().update_special_power(frame_time);
            }
        });
        result
    }

    fn is_ability_active(&self) -> bool {
        let mut active = false;
        self.entry.with_module(|module| {
            if let Some(update) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule>()
            {
                active = update.behavior_mut().is_active();
            }
        });
        active
    }
}

impl ModuleExitInterfaceProxy {
    fn with_exit_behavior<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn ExitInterface) -> R,
    {
        self.entry.with_module(|module| {
            module_production_behavior_kind(module)
                .and_then(ProductionBehaviorModuleKindMut::into_exit_interface)
                .map(func)
        })
    }
}

impl ExitInterface for ExitInterfaceProxy {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.can_exit(object_id);
            }
        }
        false
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit(object_id);
            }
        }
        false
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.get_rally_point();
            }
        }
        Ok(None)
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.reserve_door_for_exit(spawner, spawn);
            }
        }
        crate::modules::DOOR_NONE_AVAILABLE
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                exit_interface.unreserve_door_for_exit(door);
            }
        }
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface
                    .exit_object_via_door(obj.read().map(|g| g.get_id()).unwrap_or(0), door);
            }
        }
        Ok(())
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_in_a_hurry(obj_id);
            }
        }
        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        obj_id: ObjectID,
        host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.lock() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_by_budding(obj_id, host_id);
            }
        }
        Ok(())
    }
}

impl ExitInterface for ContainExitInterfaceProxy {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.contain
            .lock()
            .map(|guard| guard.can_exit(object_id))
            .unwrap_or(false)
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
            return false;
        };
        self.exit_object_via_door(
            obj.read().map(|g| g.get_id()).unwrap_or(0),
            crate::modules::ExitDoorType::Primary,
        )
        .is_ok()
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .contain
            .lock()
            .ok()
            .and_then(|guard| guard.get_rally_point()))
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        self.contain
            .lock()
            .map(|mut guard| guard.reserve_door_for_exit(spawner, spawn))
            .unwrap_or(crate::modules::DOOR_NONE_AVAILABLE)
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        if let Ok(mut guard) = self.contain.lock() {
            guard.unreserve_door_for_exit(door);
        }
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.contain
            .lock()
            .map_err(|_| "failed to lock contain exit interface".into())
            .and_then(|mut guard| {
                guard.exit_object_via_door(obj.read().map(|g| g.get_id()).unwrap_or(0), door)
            })
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.contain
            .lock()
            .map_err(|_| "failed to lock contain exit interface".into())
            .and_then(|mut guard| guard.exit_object_in_a_hurry(obj_id))
    }
}

impl ExitInterface for ModuleExitInterfaceProxy {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.with_exit_behavior(|module| module.can_exit(object_id))
            .unwrap_or(false)
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        self.with_exit_behavior(|module| module.exit(object_id))
            .unwrap_or(false)
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .with_exit_behavior(|module| module.get_rally_point())
            .transpose()?
            .flatten())
    }

    fn get_exit_position(&self, exit_position: &mut Coord3D) -> bool {
        self.with_exit_behavior(|module| module.get_exit_position(exit_position))
            .unwrap_or(false)
    }

    fn get_natural_rally_point(&self, rally_point: &mut Coord3D, offset: bool) -> bool {
        self.with_exit_behavior(|module| module.get_natural_rally_point(rally_point, offset))
            .unwrap_or(false)
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        self.with_exit_behavior(|module| module.reserve_door_for_exit(spawner, spawn))
            .unwrap_or(crate::modules::DOOR_NONE_AVAILABLE)
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        let _ = self.with_exit_behavior(|module| {
            module.unreserve_door_for_exit(door);
        });
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.with_exit_behavior(|module| {
            module.exit_object_via_door(obj.read().map(|g| g.get_id()).unwrap_or(0), door)
        })
        .unwrap_or(Ok(()))
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.with_exit_behavior(|module| module.exit_object_in_a_hurry(obj_id))
            .unwrap_or(Ok(()))
    }

    fn exit_object_by_budding(
        &mut self,
        obj_id: ObjectID,
        host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.with_exit_behavior(|module| module.exit_object_by_budding(obj_id, host_id))
            .unwrap_or(Ok(()))
    }
}

impl ObjectLockExt for Arc<RwLock<Object>> {
    fn lock(&self) -> std::sync::LockResult<std::sync::RwLockWriteGuard<'_, Object>> {
        self.write()
    }

    fn try_lock(&self) -> std::sync::TryLockResult<std::sync::RwLockWriteGuard<'_, Object>> {
        self.try_write()
    }
}

#[cfg(test)]
use crate::object::body::active_body::{ActiveBody, ActiveBodyModuleData};

pub struct ModuleEntry {
    name: AsciiString,
    tag: AsciiString,
    interface_mask: ModuleInterfaceType,
    module_data: Arc<dyn ModuleData>,
    module: Mutex<Box<dyn Module>>,
}

impl fmt::Debug for ModuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleEntry")
            .field("name", &self.name)
            .field("tag", &self.tag)
            .field("interface_mask", &self.interface_mask)
            .finish()
    }
}

impl ModuleEntry {
    fn new(
        name: AsciiString,
        tag: AsciiString,
        interface_mask: ModuleInterfaceType,
        module_data: Arc<dyn ModuleData>,
        module: Box<dyn Module>,
    ) -> Self {
        Self {
            name,
            tag,
            interface_mask,
            module_data,
            module: Mutex::new(module),
        }
    }

    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn tag(&self) -> &AsciiString {
        &self.tag
    }

    fn mask(&self) -> ModuleInterfaceType {
        self.interface_mask
    }

    fn data(&self) -> &Arc<dyn ModuleData> {
        &self.module_data
    }

    fn with_module<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        let mut guard = self.module.lock().expect("behavior module lock poisoned");
        func(guard.as_mut())
    }

    fn try_with_module<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        self.module
            .try_lock()
            .ok()
            .map(|mut guard| func(guard.as_mut()))
    }

    /// Mutable module access - same as with_module but explicitly named for clarity
    #[allow(dead_code)]
    fn with_module_mut<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        self.with_module(func)
    }

    /// Get the module name key by querying the module instance
    fn module_name_key(&self) -> NameKeyType {
        self.with_module(|module| module.get_module_name_key())
    }

    /// Get the module tag name key by querying the module instance
    fn module_tag_key(&self) -> NameKeyType {
        self.with_module(|module| module.get_module_tag_name_key())
    }
}

struct ModuleUpdateProxy {
    entry: Arc<ModuleEntry>,
    object_id: ObjectID,
    module_name: AsciiString,
}

fn module_with_downcast<T: 'static, F, R>(module: &mut dyn Module, func: F) -> Option<R>
where
    F: FnOnce(&mut T) -> R,
{
    (module as &mut dyn Any).downcast_mut::<T>().map(func)
}

fn behavior_downcast_mut<T: 'static>(behavior: &mut dyn BehaviorModuleInterface) -> Option<&mut T> {
    (behavior as &mut dyn Any).downcast_mut::<T>()
}

fn behavior_with_downcast<T: 'static, F, R>(
    behavior: &mut dyn BehaviorModuleInterface,
    func: F,
) -> Option<R>
where
    F: FnOnce(&mut T) -> R,
{
    behavior_downcast_mut::<T>(behavior).map(func)
}

impl ModuleUpdateProxy {
    fn new(entry: Arc<ModuleEntry>, object_id: ObjectID) -> Self {
        let module_name = entry.name().clone();
        Self {
            entry,
            object_id,
            module_name,
        }
    }

    fn dispatch_update(module: &mut dyn Module) -> Option<UpdateSleepTime> {
        macro_rules! update_via_behavior {
            ($ty:ty) => {
                if let Some(result) = module_with_downcast::<$ty, _, _>(module, |module| {
                    module.behavior_mut().update_simple()
                }) {
                    return Some(result);
                };
            };
        }

        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::ocl_update::OCLUpdateModule>()
        {
            return Some(module.update());
        }
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::special_power_update::SpecialPowerUpdateModule>(
        ) {
            return Some(module.update_simple());
        }
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::fire_spread_update::FireSpreadUpdateModule>(
        ) {
            return Some(module.behavior_mut().update_simple());
        }
        update_via_behavior!(crate::contain_module_overrides::ActiveBehaviorModule<
            crate::object::behavior::deletion_update::DeletionUpdate,
        >);
        update_via_behavior!(crate::contain_module_overrides::ActiveBehaviorModule<
            crate::object::behavior::animation_steering_update::AnimationSteeringUpdate,
        >);

        update_via_behavior!(crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule);
        update_via_behavior!(
            crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule
        );
        update_via_behavior!(crate::object::behavior::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModule);
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>(
        ) {
            return Some(module.update_simple());
        }
        update_via_behavior!(
            crate::object::behavior::tech_building_behavior::TechBuildingBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::propaganda_tower_behavior::PropagandaTowerBehaviorModule
        );
        #[cfg(feature = "allow_surrender")]
        {
            if let Some(module) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::propaganda_center_behavior::PropagandaCenterBehaviorModule>()
            {
                if let Some(mut guard) = module.behavior() {
                    return Some(guard.update_simple());
                }
            }
        }
        update_via_behavior!(
            crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::bridge_scaffold_behavior::BridgeScaffoldBehaviorModule
        );
        update_via_behavior!(crate::object::behavior::bridge_behavior::BridgeBehaviorModule);
        update_via_behavior!(crate::object::behavior::horde_update::HordeUpdateModule);
        update_via_behavior!(crate::object::behavior::radar_update::RadarUpdateModule);
        update_via_behavior!(crate::object::behavior::radius_decal_update::RadiusDecalUpdateModule);
        update_via_behavior!(crate::object::behavior::spawn_behavior::SpawnBehaviorModule);
        update_via_behavior!(
            crate::object::behavior::stealth_detector_update::StealthDetectorUpdateModule
        );
        update_via_behavior!(crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule);
        update_via_behavior!(crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule);
        update_via_behavior!(
            crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModule
        );
        update_via_behavior!(crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule);
        update_via_behavior!(crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehaviorModule);
        update_via_behavior!(
            crate::object::behavior::flight_deck_behavior::FlightDeckBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::parking_place_behavior::ParkingPlaceBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::rebuild_hole_behavior::RebuildHoleBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule
        );
        update_via_behavior!(
            crate::object::behavior::bunker_buster_behavior::BunkerBusterBehaviorModule
        );
        update_via_behavior!(crate::object::behavior::topple_update::ToppleUpdateModule);
        update_via_behavior!(
            crate::object::behavior::structure_topple_update::StructureToppleUpdateModule
        );
        update_via_behavior!(
            crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule
        );
        update_via_behavior!(
            crate::object::production::production_update_complete::ProductionUpdateCompleteModule
        );
        update_via_behavior!(crate::object::behavior::sticky_bomb_update::StickyBombUpdateModule);
        update_via_behavior!(crate::object::behavior::prone_update::ProneUpdateModule);
        update_via_behavior!(
            crate::object::behavior::projectile_stream_update::ProjectileStreamUpdateModule
        );
        update_via_behavior!(
            crate::object::behavior::point_defense_laser_update::PointDefenseLaserUpdateModule
        );
        update_via_behavior!(crate::object::behavior::laser_update::LaserUpdateModule);
        update_via_behavior!(crate::object::update::bone_fx_update::BoneFXUpdateModule);
        update_via_behavior!(crate::object::behavior::demo_trap_update::DemoTrapUpdateModule);
        update_via_behavior!(crate::object::behavior::smart_bomb_target_homing_update::SmartBombTargetHomingUpdateModule);
        update_via_behavior!(
            crate::object::behavior::tensile_formation_update::TensileFormationUpdateModule
        );
        update_via_behavior!(
            crate::object::behavior::generate_minefield_behavior::GenerateMinefieldBehaviorModule
        );
        update_via_behavior!(crate::object::behavior::minefield_behavior::MinefieldBehaviorModule);
        update_via_behavior!(
            crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule
        );
        update_via_behavior!(
            crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule
        );
        update_via_behavior!(crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule);
        update_via_behavior!(crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule);
        update_via_behavior!(crate::object::behavior::battle_plan_update::BattlePlanUpdateModule);
        update_via_behavior!(crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule);
        update_via_behavior!(crate::object::behavior::lifetime_update::LifetimeUpdateModule);
        update_via_behavior!(crate::object::update::spy_vision_update::SpyVisionUpdateModule);
        update_via_behavior!(crate::object::behavior::fire_weapon_when_damaged_behavior_new::FireWeaponWhenDamagedBehaviorModule);
        update_via_behavior!(crate::object::behavior::fire_weapon_update::FireWeaponUpdateModule);
        update_via_behavior!(crate::object::behavior::fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdateModule);
        update_via_behavior!(crate::object::behavior::weapon_bonus_update::WeaponBonusUpdateModule);
        update_via_behavior!(crate::object::behavior::emp_update::EMPUpdateModule);
        update_via_behavior!(
            crate::object::behavior::structure_collapse_update::StructureCollapseUpdateModule
        );
        update_via_behavior!(crate::object::behavior::float_update::FloatUpdateModule);
        update_via_behavior!(crate::object::behavior::enemy_near_update::EnemyNearUpdateModule);
        update_via_behavior!(
            crate::object::behavior::auto_find_healing_update::AutoFindHealingUpdateModule
        );
        update_via_behavior!(
            crate::object::behavior::base_regenerate_update::BaseRegenerateUpdateModule
        );
        update_via_behavior!(crate::object::behavior::auto_deposit_update::AutoDepositUpdateModule);
        update_via_behavior!(crate::object::behavior::power_plant_update::PowerPlantUpdateModule);
        update_via_behavior!(
            crate::object::behavior::assisted_targeting_update::AssistedTargetingUpdateModule
        );
        update_via_behavior!(crate::object::behavior::dynamic_shroud_clearing_range_update::DynamicShroudClearingRangeUpdateModule);
        update_via_behavior!(
            crate::object::behavior::cleanup_hazard_update::CleanupHazardUpdateModule
        );
        // C++ FlammableUpdate is an UpdateModule; without this entry its proxy
        // warns "No update dispatcher" and sleeps Forever, so the wake armed by
        // tryToIgnite (FlammableUpdate.cpp:196) can never advance the burn
        // state machine.
        update_via_behavior!(
            crate::contain_module_overrides::ActiveBehaviorModule<
                crate::object::behavior::flammable_update::FlammableUpdate,
            >
        );
        update_via_behavior!(
            crate::object::production::railed_transport_dock::RailedTransportDockUpdateModule
        );
        update_via_behavior!(
            crate::object::update::command_button_hunt_update::CommandButtonHuntUpdateModule
        );
        update_via_behavior!(crate::object::update::slaved_update::SlavedUpdateModule);
        update_via_behavior!(
            crate::object::update::mob_member_slaved_update::MobMemberSlavedUpdateModule
        );

        None
    }

    fn dispatch_disabled_mask(module: &mut dyn Module) -> Option<DisabledMaskType> {
        macro_rules! mask_via_behavior {
            ($ty:ty) => {
                if let Some(result) = module_with_downcast::<$ty, _, _>(module, |module| {
                    module.behavior_mut().get_disabled_types_to_process()
                }) {
                    return Some(result);
                };
            };
        }

        mask_via_behavior!(crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule);
        mask_via_behavior!(
            crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule
        );
        mask_via_behavior!(crate::object::behavior::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModule);
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>(
        ) {
            return Some(module.get_disabled_types_to_process());
        }
        mask_via_behavior!(
            crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehaviorModule
        );
        mask_via_behavior!(
            crate::object::behavior::bridge_scaffold_behavior::BridgeScaffoldBehaviorModule
        );
        mask_via_behavior!(crate::object::behavior::bridge_behavior::BridgeBehaviorModule);
        mask_via_behavior!(crate::object::behavior::horde_update::HordeUpdateModule);
        mask_via_behavior!(crate::object::behavior::radar_update::RadarUpdateModule);
        mask_via_behavior!(crate::object::behavior::radius_decal_update::RadiusDecalUpdateModule);
        mask_via_behavior!(crate::object::behavior::spawn_behavior::SpawnBehaviorModule);
        mask_via_behavior!(
            crate::object::behavior::stealth_detector_update::StealthDetectorUpdateModule
        );
        mask_via_behavior!(crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule);
        mask_via_behavior!(crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule);
        mask_via_behavior!(
            crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModule
        );
        mask_via_behavior!(crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule);
        mask_via_behavior!(crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehaviorModule);
        mask_via_behavior!(crate::object::behavior::flight_deck_behavior::FlightDeckBehaviorModule);
        mask_via_behavior!(
            crate::object::behavior::rebuild_hole_behavior::RebuildHoleBehaviorModule
        );
        mask_via_behavior!(crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule);
        mask_via_behavior!(
            crate::object::behavior::bunker_buster_behavior::BunkerBusterBehaviorModule
        );
        mask_via_behavior!(crate::object::behavior::topple_update::ToppleUpdateModule);
        mask_via_behavior!(
            crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule
        );
        mask_via_behavior!(
            crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule
        );
        mask_via_behavior!(crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule);
        mask_via_behavior!(crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule);
        mask_via_behavior!(crate::object::behavior::battle_plan_update::BattlePlanUpdateModule);
        mask_via_behavior!(crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule);
        mask_via_behavior!(crate::object::behavior::lifetime_update::LifetimeUpdateModule);
        mask_via_behavior!(
            crate::object::update::ai_update::railroad_guide_ai_update::RailroadBehaviorModule
        );
        mask_via_behavior!(
            crate::object::production::production_update_complete::ProductionUpdateCompleteModule
        );
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::special_power_update::SpecialPowerUpdateModule>(
        ) {
            return Some(module.get_disabled_types_to_process());
        }

        None
    }

    fn dispatch_phase(module: &mut dyn Module) -> Option<SleepyUpdatePhase> {
        macro_rules! phase_via_behavior {
            ($ty:ty) => {
                if let Some(result) = module_with_downcast::<$ty, _, _>(module, |module| {
                    module.behavior_mut().get_update_phase()
                }) {
                    return Some(result);
                };
            };
        }

        phase_via_behavior!(crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule);
        phase_via_behavior!(
            crate::object::behavior::firing_tracker_behavior::FiringTrackerBehaviorModule
        );
        phase_via_behavior!(crate::object::behavior::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModule);
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>(
        ) {
            return Some(module.get_update_phase());
        }
        phase_via_behavior!(
            crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehaviorModule
        );
        phase_via_behavior!(
            crate::object::behavior::bridge_scaffold_behavior::BridgeScaffoldBehaviorModule
        );
        phase_via_behavior!(crate::object::behavior::bridge_behavior::BridgeBehaviorModule);
        phase_via_behavior!(crate::object::behavior::horde_update::HordeUpdateModule);
        phase_via_behavior!(crate::object::behavior::radar_update::RadarUpdateModule);
        phase_via_behavior!(crate::object::behavior::radius_decal_update::RadiusDecalUpdateModule);
        phase_via_behavior!(crate::object::behavior::spawn_behavior::SpawnBehaviorModule);
        phase_via_behavior!(
            crate::object::behavior::stealth_detector_update::StealthDetectorUpdateModule
        );
        phase_via_behavior!(crate::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitBehaviorModule);
        phase_via_behavior!(crate::object::behavior::supply_center_production_exit_behavior::SupplyCenterProductionExitBehaviorModule);
        phase_via_behavior!(
            crate::object::behavior::countermeasures_behavior::CountermeasuresBehaviorModule
        );
        phase_via_behavior!(crate::object::behavior::default_production_exit_behavior::DefaultProductionExitBehaviorModule);
        phase_via_behavior!(crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehaviorModule);
        phase_via_behavior!(
            crate::object::behavior::flight_deck_behavior::FlightDeckBehaviorModule
        );
        phase_via_behavior!(
            crate::object::behavior::rebuild_hole_behavior::RebuildHoleBehaviorModule
        );
        phase_via_behavior!(crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule);
        phase_via_behavior!(
            crate::object::behavior::bunker_buster_behavior::BunkerBusterBehaviorModule
        );
        phase_via_behavior!(crate::object::behavior::topple_update::ToppleUpdateModule);
        phase_via_behavior!(
            crate::object::behavior::special_ability_update::SpecialAbilityUpdateModule
        );
        phase_via_behavior!(
            crate::object::behavior::spectre_gunship_update::SpectreGunshipUpdateModule
        );
        phase_via_behavior!(crate::object::behavior::spectre_gunship_deployment_update::SpectreGunshipDeploymentUpdateModule);
        phase_via_behavior!(crate::object::behavior::particle_uplink_cannon_update::ParticleUplinkCannonUpdateModule);
        phase_via_behavior!(crate::object::behavior::battle_plan_update::BattlePlanUpdateModule);
        phase_via_behavior!(crate::object::behavior::missile_launcher_building_update::MissileLauncherBuildingUpdateModule);
        phase_via_behavior!(crate::object::behavior::lifetime_update::LifetimeUpdateModule);
        if let Some(module) = (module as &mut dyn Any)
            .downcast_mut::<crate::object::update::special_power_update::SpecialPowerUpdateModule>(
        ) {
            return Some(module.get_update_phase());
        }

        None
    }
}

fn initial_update_wake_frame(entry: &ModuleEntry) -> UnsignedInt {
    entry.with_module(|module| {
        module
            .as_any()
            .downcast_ref::<crate::object::behavior::lifetime_update::LifetimeUpdateModule>()
            .map(|module| module.initial_wake_frame())
            .or_else(|| {
                module
                    .as_any()
                    .downcast_ref::<crate::contain_module_overrides::ActiveBehaviorModule<
                        crate::object::behavior::deletion_update::DeletionUpdate,
                    >>()
                    .map(|module| module.behavior().initial_wake_frame())
            })
            .or_else(|| {
                // C++ SlowDeathBehavior ctor setWakeFrame(UPDATE_SLEEP_FOREVER)
                // until beginSlowDeath.
                module
                    .as_any()
                    .downcast_ref::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>()
                    .map(|_| UpdateSleepTime::Forever.to_u32())
            })
            .unwrap_or(0)
    })
}

impl UpdateModuleInterface for ModuleUpdateProxy {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let mut sleep = None;
        self.entry.with_module(|module| {
            sleep = Self::dispatch_update(module);
        });

        if let Some(sleep) = sleep {
            return Ok(sleep);
        }

        warn!(
            "No update dispatcher for module '{}' on object {}",
            self.module_name, self.object_id
        );
        Ok(UpdateSleepTime::Forever)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        let mut mask = None;
        self.entry.with_module(|module| {
            mask = Self::dispatch_disabled_mask(module);
        });
        mask.unwrap_or_else(DisabledMaskType::none)
    }

    fn get_update_phase(&self) -> SleepyUpdatePhase {
        let mut phase = None;
        self.entry.with_module(|module| {
            phase = Self::dispatch_phase(module);
        });
        phase.unwrap_or(SleepyUpdatePhase::Normal)
    }
}

#[derive(Debug, Clone)]
pub struct BehaviorModuleHandle {
    entry: Arc<ModuleEntry>,
}

impl BehaviorModuleHandle {
    fn new(entry: Arc<ModuleEntry>) -> Self {
        Self { entry }
    }

    pub fn name(&self) -> &AsciiString {
        self.entry.name()
    }

    pub fn tag(&self) -> &AsciiString {
        self.entry.tag()
    }

    pub fn interface_mask(&self) -> ModuleInterfaceType {
        self.entry.mask()
    }

    pub fn with_module<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        self.entry.with_module(func)
    }

    pub fn try_with_module<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        self.entry.try_with_module(func)
    }

    pub fn with_module_data<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&dyn ModuleData) -> R,
    {
        func(self.entry.data().as_ref())
    }

    pub fn module_data_arc(&self) -> Arc<dyn ModuleData> {
        Arc::clone(self.entry.data())
    }

    pub fn module_name_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_name_key())
    }

    pub fn module_tag_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_tag_name_key())
    }

    pub fn with_module_downcast<T: 'static, F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.entry
            .with_module(|module| module_with_downcast::<T, _, _>(module, func))
    }
}

#[derive(Clone)]
struct BehaviorModuleProxy {
    entry: Arc<ModuleEntry>,
}

impl BehaviorModuleProxy {
    fn new(entry: Arc<ModuleEntry>) -> Self {
        Self { entry }
    }
}

impl EngineSnapshotable for BehaviorModuleProxy {
    fn crc(&self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.entry.with_module(|module| module.crc(xfer))
    }

    fn xfer(&mut self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.entry.with_module(|module| module.xfer(xfer))
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.entry.with_module(|module| module.load_post_process())
    }
}

impl engine_module::Module for BehaviorModuleProxy {
    fn get_module_name_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_name_key())
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_tag_name_key())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.entry.data().as_ref()
    }

    fn on_object_created(&mut self) {
        self.entry.with_module(|module| module.on_object_created());
    }

    fn on_drawable_bound_to_object(&mut self) {
        self.entry
            .with_module(|module| module.on_drawable_bound_to_object());
    }

    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.entry
            .with_module(|module| module.preload_assets(time_of_day));
    }

    fn on_delete(&mut self) {
        self.entry.with_module(|module| module.on_delete());
    }
}

// Constants
pub const MAX_TRIGGER_AREA_INFOS: usize = 5;
pub const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;
pub const WEAPONSLOT_COUNT: usize = 3;
pub const DISABLED_COUNT: usize = 13;
pub const NUM_SLEEP_HELPERS: usize = 8;
/// C++ `BuildAssistant.h:25` `enum { CONSTRUCTION_COMPLETE = -1 }`.
pub const CONSTRUCTION_COMPLETE: Real = -1.0;
pub const NEVER: UnsignedInt = 0xFFFFFFFF;
pub const INVALID_ID: ObjectID = 0;

// Enumerations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrushSquishTestType {
    TestCrushOnly,
    TestSquishOnly,
    TestCrushOrSquish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectScriptStatusBit {
    /// This object is disabled via script.
    ScriptDisabled = 0x01,
    /// This object is unpowered via script.
    ScriptUnderpowered = 0x02,
    /// Prevents selling (used by scripts/cinematics and AI capture edge cases).
    Unsellable = 0x04,
    /// Marks an object as forcibly unstealthed by script.
    ScriptUnstealthed = 0x08,
    /// Allows scripts to target the object even if normal targeting would not.
    ScriptTargetable = 0x10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Private status bits for Object
#[repr(u8)]
enum ObjectPrivateStatusBits {
    EffectivelyDead = 1 << 0,
    UndetectedDefector = 1 << 1,
    Captured = 1 << 2,
    OffMap = 1 << 3,
}

fn disabled_type_from_index(index: usize) -> Option<DisabledType> {
    match index {
        0 => Some(DisabledType::DisabledDefault),
        1 => Some(DisabledType::DisabledHacked),
        2 => Some(DisabledType::DisabledEmp),
        3 => Some(DisabledType::Held),
        4 => Some(DisabledType::Paralyzed),
        5 => Some(DisabledType::DisabledUnmanned),
        6 => Some(DisabledType::DisabledUnderpowered),
        7 => Some(DisabledType::DisabledFreefall),
        8 => Some(DisabledType::DisabledAwestruck),
        9 => Some(DisabledType::DisabledBrainwashed),
        10 => Some(DisabledType::DisabledSubdued),
        11 => Some(DisabledType::DisabledScriptDisabled),
        12 => Some(DisabledType::DisabledScriptUnderpowered),
        _ => None,
    }
}

/// Trigger area information structure
#[derive(Debug, Clone)]
pub struct TriggerInfo {
    pub trigger: Option<Arc<PolygonTrigger>>,
    pub entered: bool,
    pub exited: bool,
    pub is_inside: bool,
}

impl Default for TriggerInfo {
    fn default() -> Self {
        Self {
            trigger: None,
            entered: false,
            exited: false,
            is_inside: false,
        }
    }
}

/// Sighting information for partition management
#[derive(Debug, Clone)]
pub struct SightingInfo {
    where_pos: Coord3D,
    how_far: Real,
    for_whom: PlayerMaskType,
    data: UnsignedInt,
}

impl SightingInfo {
    pub fn new() -> Self {
        Self {
            where_pos: Coord3D::new(0.0, 0.0, 0.0),
            how_far: 0.0,
            for_whom: PlayerMaskType::none(),
            data: 0,
        }
    }

    pub fn reset(&mut self) {
        self.where_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.how_far = 0.0;
        self.for_whom = PlayerMaskType::none();
        self.data = 0;
    }

    pub fn is_invalid(&self) -> bool {
        self.how_far == 0.0
    }
}

/// Radar object data (shared with the Common radar system).
pub type RadarObject = game_engine::common::system::radar::RadarObject;

// PartitionData lives in `partition_data.rs` (C++ PartitionData::getShroudedStatus).

/// Polygon trigger for area detection.
pub use crate::polygon_trigger::PolygonTrigger;

/// Waypoint for movement and targeting.
pub use crate::waypoint::Waypoint;

/// Command button for UI interaction.
pub use crate::command_button::CommandButton;

pub use die::special_power_completion_die::SpecialPowerCompletionDie;
pub use special_power_template::SpecialPowerTemplate;

/// Subset of model condition flags required by the current port. The original
/// C++ enum is far larger; we expand this as behaviors require.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelConditionFlagType {
    StunnedFlailing,
    ArmorsetCrateUpgradeOne,
    ArmorsetCrateUpgradeTwo,
    Captured,
}

/// Errors that can occur during Object operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum ObjectError {
    #[error("Object is already dead")]
    AlreadyDead,

    #[error("Invalid damage amount: {0}")]
    InvalidDamage(f32),

    #[error("Object is invulnerable to this damage")]
    Invulnerable,

    #[error("Object has no body module")]
    NoBodyModule,

    #[error("Body module has indestructible body")]
    IndestructibleBody,

    #[error("Lock was poisoned")]
    LockPoisoned,

    #[error("Body module error: {0}")]
    BodyModuleError(String),

    #[error("No weapon available")]
    NoWeapon,

    #[error("Weapon is not ready to fire")]
    WeaponNotReady,

    #[error("Target is invalid or destroyed")]
    TargetInvalid,

    #[error("Weapon fire failed: {0}")]
    WeaponFireFailed(String),

    #[error("Physics system not available")]
    NoPhysicsSystem,

    #[error("Invalid object state")]
    InvalidState,
}

/// Main Object struct - the core game entity
#[allow(dead_code)]
pub struct Object {
    // Core identification
    id: ObjectID,
    producer_id: ObjectID,
    builder_id: ObjectID,
    name: AsciiString,
    thing_template: Arc<dyn ThingTemplate>,

    // Intrusive list shadow links for efficient iteration.
    // C++ stores raw pointers here; Rust keeps IDs and resolves through the registry.
    next_object_id: Option<ObjectID>,
    prev_object_id: Option<ObjectID>,

    // Status and state
    status: ObjectStatusMaskType,
    private_status: u8,
    script_status: u8,

    // Geometry and position
    geometry_info: GeometryInfo,
    health_box_offset: Coord3D,
    i_pos: ICoord3D,

    // Team and ownership (ID-first; pin only when team is not factory-registered)
    team_id: Option<TeamID>,
    team_pin: Option<Arc<RwLock<Team>>>,
    original_team_name: AsciiString,
    indicator_color: Color,

    // Modules - using Arc<Mutex<>> for thread safety
    behaviors: Vec<Arc<Mutex<dyn BehaviorModuleInterface>>>,
    modules: Vec<Arc<ModuleEntry>>,
    body_module_handles: Vec<Arc<ModuleEntry>>,
    die_module_handles: Vec<Arc<ModuleEntry>>,
    update_module_handles: Vec<Arc<ModuleEntry>>,
    update_module_registrations: Vec<UpdateModulePtr>,
    collide_module_handles: Vec<Arc<ModuleEntry>>,
    contain_module_handles: Vec<Arc<ModuleEntry>>,
    upgrade_module_handles: Vec<Arc<ModuleEntry>>,
    body: Option<Arc<Mutex<dyn BodyModuleInterface>>>,
    contain: Option<Arc<Mutex<dyn ContainModuleInterface>>>,
    stealth: Option<StealthUpdateHandle>,
    ai: Option<Arc<Mutex<dyn AIUpdateInterface>>>,
    physics: Option<Arc<Mutex<dyn PhysicsBehavior>>>,

    // Helper modules
    repulsor_helper: Option<Arc<Mutex<ObjectRepulsorHelper>>>,
    smc_helper: Option<Arc<Mutex<ObjectSMCHelper>>>,
    ws_helper: Option<Arc<Mutex<ObjectWeaponStatusHelper>>>,
    defection_helper: Option<Arc<Mutex<ObjectDefectionHelper>>>,
    status_damage_helper: Option<Arc<Mutex<StatusDamageHelper>>>,
    subdual_damage_helper: Option<Arc<Mutex<SubdualDamageHelper>>>,
    temp_weapon_bonus_helper: Option<Arc<Mutex<TempWeaponBonusHelper>>>,
    firing_tracker: Option<Arc<Mutex<FiringTracker>>>,
    held_helper: Option<Arc<Mutex<ObjectHeldHelper>>>,

    // Spatial and partition data
    partition_data: Option<Arc<Mutex<PartitionData>>>,
    radar_data: Option<Arc<Mutex<RadarObject>>>,

    // Vision and detection
    partition_last_look: SightingInfo,
    partition_reveal_all_last_look: SightingInfo,
    partition_last_shroud: SightingInfo,
    partition_last_threat: SightingInfo,
    partition_last_value: SightingInfo,
    vision_spied_by: [i32; MAX_PLAYER_COUNT],
    vision_spied_mask: PlayerMaskType,
    vision_range: Real,
    shroud_clearing_range: Real,
    shroud_range: Real,

    // Containment
    /// Container object id (INVALID_ID if not contained).
    contained_by_id: ObjectID,
    contained_by_frame: UnsignedInt,
    is_transporting: Bool,

    // Construction and upgrades
    construction_percent: Real,
    object_upgrades_completed: UpgradeMaskType,

    // Group membership
    group_id: Option<u32>,

    // Experience and combat
    experience_tracker: Option<Arc<Mutex<ExperienceTracker>>>,
    captured: bool,
    veterancy_level: VeterancyLevel,
    experience_points: Real,

    // Weapons and combat
    pub weapon_set: WeaponSet,
    /// Multiplicative weapon bonus (e.g., upgrades/veterancy). 1.0 = none.
    weapon_bonus_multiplier: f32,
    cur_weapon_set_flags: WeaponSetFlags,
    armor_set_flags: ArmorSetFlagBits,
    weapon_bonus_condition: WeaponBonusConditionFlags,
    last_weapon_condition: [u8; WEAPONSLOT_COUNT],
    special_power_bits: SpecialPowerMask,

    // Healing tracking (for non-stacking healers)
    sole_healing_benefactor_id: ObjectID,
    sole_healing_benefactor_expiration_frame: UnsignedInt,

    // Disabled states
    disabled_mask: DisabledMaskType,
    disabled_till_frame: [UnsignedInt; DISABLED_COUNT],
    smc_until: UnsignedInt,
    special_model_condition_flag: ModelConditionFlags,
    invulnerable_until_frame: UnsignedInt,

    // Trigger areas
    trigger_info: [TriggerInfo; MAX_TRIGGER_AREA_INFOS],
    entered_or_exited_frame: UnsignedInt,
    num_trigger_areas_active: u8,

    // Pathfinding
    layer: PathfindLayerEnum,
    destination_layer: PathfindLayerEnum,

    // Formation
    formation_id: FormationID,
    formation_offset: Coord2D,

    // Command overrides
    command_set_string_override: AsciiString,

    // Rendering
    safe_occlusion_frame: UnsignedInt,
    carrier_deck_height: Real,

    // Drawable association
    drawable: Option<Arc<RwLock<Drawable>>>,

    // Visibility flags for rendering (per-player fog-of-war)
    // Track which players can see this object for rendering optimization
    visibility_flags: [bool; MAX_PLAYER_COUNT],
    visibility_alpha: [f32; MAX_PLAYER_COUNT], // Alpha blending for partial visibility
    last_visibility_update_frame: UnsignedInt,

    // Flags
    is_selectable: bool,
    modules_ready: bool,
    single_use_command_used: bool,
    is_receiving_difficulty_bonus: bool,

    /// Guard flag to prevent double destruction when `on_destroy()` is called
    /// both explicitly and via `Drop`.
    destroyed: bool,

    #[cfg(any(debug_assertions, feature = "internal"))]
    has_died_already: bool,
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("template", &self.thing_template.get_name())
            .finish()
    }
}

enum UpgradeModuleHandle {
    StatusBits(StatusBitsUpgradeHandle),
    PassengersFire(PassengersFireUpgradeHandle),
    SubObjects(SubObjectsUpgradeHandle),
}

#[derive(Debug, Clone, Copy, Default)]
struct ArmorSetFlagBits(u32);

impl ArmorSetFlagBits {
    fn set(&mut self, flag: ArmorSetFlag) {
        self.0 |= 1 << (flag as u8);
    }

    fn clear(&mut self, flag: ArmorSetFlag) {
        self.0 &= !(1 << (flag as u8));
    }

    fn test(&self, flag: ArmorSetFlag) -> bool {
        (self.0 & (1 << (flag as u8))) != 0
    }
}

/// Flags used by salvage armor upgrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSetFlag {
    CrateUpgradeOne = 0,
    CrateUpgradeTwo = 1,
}

fn armor_set_type_for_flag(flag: ArmorSetFlag) -> crate::object::body::body_module::ArmorSetType {
    match flag {
        ArmorSetFlag::CrateUpgradeOne => {
            crate::object::body::body_module::ArmorSetType::CrateUpgradeOne
        }
        ArmorSetFlag::CrateUpgradeTwo => {
            crate::object::body::body_module::ArmorSetType::CrateUpgradeTwo
        }
    }
}

fn weapon_set_model_condition(flag: WeaponSetType) -> Option<ModelConditionFlags> {
    match flag {
        WeaponSetType::Veteran => Some(ModelConditionFlags::WEAPONSET_VETERAN),
        WeaponSetType::Elite => Some(ModelConditionFlags::WEAPONSET_ELITE),
        WeaponSetType::Hero => Some(ModelConditionFlags::WEAPONSET_HERO),
        WeaponSetType::PlayerUpgrade => Some(ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE),
        WeaponSetType::CrateUpgradeOne => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_ONE),
        WeaponSetType::CrateUpgradeTwo => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_TWO),
        _ => None,
    }
}

// Inherent Object methods and later trait impls live in sibling files.
mod capture;
mod command_buttons;
mod command_weapon;
mod die_hooks;
mod disabled;
mod entity_module_host;
mod init;
mod object_combat;
mod object_impl_imports;
mod object_lifecycle;
mod object_modules;
mod object_queries;
mod object_special_power;
mod object_status;
#[cfg(test)]
mod object_tests;
mod object_thing;
mod object_triggers;
mod object_update;
mod object_upgrade;
mod object_vision;
mod object_xfer;
mod status_cmds;
mod vision;

pub use object_thing::ObjectArcExt;
pub(crate) use object_thing::{ObjectThingHandle, make_drawable_module_thing_handle};

pub type ObjectId = ObjectID;
