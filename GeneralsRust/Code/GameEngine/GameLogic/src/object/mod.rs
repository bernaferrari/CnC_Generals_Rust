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
pub mod update_module_interfaces;
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
    PartitionData, partition_cell_shroud_counts, partition_cell_shroud_status,
    stamp_partition_cell_covers, stamp_partition_cell_lookers,
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
    behavior: BehaviorInterfaceHandle,
}

#[allow(dead_code)]
struct ModuleSpecialAbilityUpdateProxy {
    entry: Arc<ModuleEntry>,
}

struct ExitInterfaceProxy {
    behavior: BehaviorInterfaceHandle,
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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

pub(crate) enum ProductionBehaviorQueueKindMut<'a> {
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
                                let facts =
                                    crate::helpers::capture_player_audio_locality(&player, &list);
                                player.get_money_mut_with_locality(facts).add_money(credits);
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
    behavior.as_production_queue_kind()
}

pub(crate) enum ProductionBehaviorRallyKindMut<'a> {
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
    behavior.as_production_rally_kind()
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
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(update) = guard.get_special_power_update_interface() {
                return update.update_special_power(frame_time);
            }
        }
        Ok(())
    }

    fn is_ability_active(&self) -> bool {
        let mut behavior = self.behavior.clone();
        if let Ok(mut guard) = behavior.access() {
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
        let mut behavior = self.behavior.clone();
        if let Ok(mut guard) = behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.can_exit(object_id);
            }
        }
        false
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit(object_id);
            }
        }
        false
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        let mut behavior = self.behavior.clone();
        if let Ok(mut guard) = behavior.access() {
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
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.reserve_door_for_exit(spawner, spawn);
            }
        }
        crate::modules::DOOR_NONE_AVAILABLE
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        if let Ok(mut guard) = self.behavior.access() {
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

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_via_door(obj_id, door);
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

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.access() {
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

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.access() {
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
        let exit_id = obj.read().map(|g| g.get_id()).unwrap_or(0);
        self.exit_object_via_door(exit_id, crate::modules::ExitDoorType::Primary)
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
            .unwrap_or(crate::modules::ExitDoorType::NoneAvailable)
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

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        self.contain
            .lock()
            .map_err(|_| "failed to lock contain exit interface".into())
            .and_then(|mut guard| guard.exit_object_via_door(obj_id, door))
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
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

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        self.with_exit_behavior(|module| module.exit_object_via_door(obj_id, door))
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

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
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

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
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

    /// Sleepy-update dispatch. Every branch of the former per-type downcast
    /// tables is now a `Module::get_update_module_interface()` override (see
    /// `super::update_module_interfaces`); the `None` cases are unchanged, so
    /// the "No update dispatcher" warning below still fires for exactly the
    /// same modules.
    fn dispatch_update(module: &mut dyn Module) -> Option<UpdateSleepTime> {
        if let Some(sleep) = module
            .get_update_module_interface()
            .map(|update| update.update_simple())
        {
            return Some(sleep);
        }
        module.update_module_behind_shared_lock()
    }

    fn dispatch_disabled_mask(module: &mut dyn Module) -> Option<DisabledMaskType> {
        module
            .get_sleepy_update_interface()
            .map(|update| update.get_disabled_types_to_process())
    }

    fn dispatch_phase(module: &mut dyn Module) -> Option<SleepyUpdatePhase> {
        module
            .get_sleepy_update_interface()
            .map(|update| update.get_update_phase())
    }
}

fn initial_update_wake_frame(entry: &ModuleEntry) -> UnsignedInt {
    // Every branch of the former `as_any().downcast_ref` chain is now a
    // `Module::get_initial_wake_frame()` override (see the wrapper modules);
    // modules without a wake frame still report 0 here.
    entry.with_module(|module| module.get_initial_wake_frame().unwrap_or(0))
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

    fn module_name(&self) -> &str {
        self.module_name.as_str()
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
    pub fn module_name(&self) -> &str {
        self.name().as_str()
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
pub use crate::common::CONSTRUCTION_COMPLETE;
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

/// Exit path queued while this object's AI mutex is already held.
/// Applied by that unit's `UnitAIUpdate::update`, not a process-global slot.
#[derive(Debug)]
pub(crate) enum PendingProducedExit {
    Quick(Vec<Coord3D>),
    Follow {
        path: Vec<Coord3D>,
        ignore_id: ObjectID,
        end: Coord3D,
    },
}

/// Immediate GameLogic-owned work emitted while an Object is being destroyed.
pub(crate) enum ObjectDestroyServiceAction {
    UnregisterUpdateModule(UpdateModulePtr),
    MarkTriggerAreasChanged,
    NotifyObjectCountChanged,
    SendObjectDestroyed,
}

/// Runtime cleanup phase; save data continues to use the C++ status bits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ObjectLifecycle {
    Alive,
    DestroyNotified,
    Finalized,
}

/// Main Object struct - the core game entity
#[allow(dead_code)]
pub struct Object {
    // Core identification
    id: ObjectID,
    producer_id: ObjectID,
    /// Copied from the AI before set_position while that mutex is already held.
    pub cached_main_turret_yaw: f32,
    pub cached_main_turret_pitch: f32,
    pub cached_main_turret_valid: bool,
    /// Copied from UnitAI before the state machine runs. The machine already holds that mutex.
    pub(crate) ai_fire_attack_ok: bool,
    pub(crate) ai_fire_turrets_linked: bool,
    pub(crate) ai_fire_has_primary: bool,
    pub(crate) ai_fire_has_secondary: bool,
    pub(crate) ai_fire_primary_enabled: bool,
    pub(crate) ai_fire_secondary_enabled: bool,
    pub(crate) ai_fire_current_victim: Option<ObjectID>,
    pub(crate) ai_fire_original_victim_pos: Option<Coord3D>,
    pub(crate) ai_fire_last_command_source: crate::common::CommandSourceType,
    pub(crate) ai_fire_mood_value: u32,
    pub(crate) ai_fire_pending_victim: Option<ObjectID>,
    pub(crate) ai_fire_which_turret: crate::common::TurretType,
    pub(crate) ai_fire_primary_turn_rate: f32,
    pub(crate) ai_fire_secondary_turn_rate: f32,
    pub(crate) ai_pending_desired_speed: Option<f32>,
    pub(crate) ai_fire_state_id: Option<u32>,
    pub(crate) ai_fire_mood_target: Option<ObjectID>,
    pub(crate) ai_fire_ground_movement: bool,
    pub(crate) ai_fire_can_turn_in_place: bool,
    pub(crate) ai_fire_is_idle: bool,
    pub(crate) ai_fire_ultra_accurate: bool,
    pub(crate) ai_fire_next_mood_check: u32,
    pub(crate) ai_fire_idle_mood_adjust: u32,
    pub(crate) ai_fire_crate_id: ObjectID,
    pub(crate) ai_fire_idle_attack_target: Option<ObjectID>,
    pub(crate) ai_pending_move_crate: Option<ObjectID>,
    pub(crate) ai_pending_attack_id: Option<ObjectID>,
    /// Mood attack-move queued while the state machine is already borrowed.
    pub(crate) ai_pending_attack_move: Option<Coord3D>,
    /// Mood attack-follow waypoint, applied after the machine guard drops.
    pub(crate) ai_pending_attack_follow_waypoint: Option<crate::waypoint::WaypointId>,
    pub(crate) ai_pending_attack_follow_as_team: bool,
    /// State to enter after this step drops the machine mutex.
    pub(crate) ai_pending_state_id: Option<u32>,
    pub(crate) ai_pending_clear_guard_target: bool,
    pub(crate) ai_pending_wake_path: bool,
    pub(crate) ai_pending_clear_move_out: bool,
    pub(crate) ai_fire_locomotor_speed: f32,
    pub(crate) ai_fire_blocked_and_stuck: bool,
    pub(crate) ai_fire_has_path_destination: bool,
    pub(crate) ai_fire_path_destination: Option<Coord3D>,
    pub(crate) ai_fire_loco_appearance: Option<crate::locomotor::LocomotorAppearance>,
    pub(crate) ai_pending_ending_move: bool,
    pub(crate) ai_fire_is_moving: bool,
    pub(crate) ai_fire_waypoint_queue_empty: bool,
    pub(crate) ai_pending_completed_waypoint: Option<crate::waypoint::WaypointId>,
    pub(crate) ai_pending_precise_z: Option<bool>,
    pub(crate) ai_pending_goal_path_index: Option<i32>,
    pub(crate) ai_pending_busy: bool,
    pub(crate) ai_fire_in_rappel: bool,
    pub(crate) ai_pending_combat_drop: bool,
    pub(crate) ai_pending_hack: bool,
    pub(crate) ai_pending_hack_source: crate::common::CommandSourceType,
    pub(crate) ai_pending_idle: bool,
    pub(crate) ai_pending_idle_source: crate::common::CommandSourceType,
    pub(crate) ai_pending_exit: Option<bool>,
    pub(crate) ai_pending_exit_source: crate::common::CommandSourceType,
    pub(crate) ai_pending_exit_obj: Option<crate::object::ObjectId>,
    /// Door/hurry exit paths queued because this object's AI mutex was already held.
    pub(crate) ai_pending_produced_exits: Vec<PendingProducedExit>,
    pub(crate) ai_fire_hacking: bool,
    pub(crate) ai_fire_hack_known: bool,
    pub(crate) ai_fire_combat_drop: bool,
    pub(crate) ai_fire_desired_speed: f32,
    pub(crate) ai_pending_rappel: bool,
    pub(crate) ai_pending_follow_pos: Option<crate::common::Coord3D>,
    pub(crate) ai_pending_heal: Option<crate::object::ObjectId>,
    pub(crate) ai_pending_evacuate: bool,
    pub(crate) ai_pending_rappel_obj: Option<crate::object::ObjectId>,
    pub(crate) ai_pending_rappel_pos: Option<crate::common::Coord3D>,
    pub(crate) ai_pending_combat_drop_obj: Option<crate::object::ObjectId>,
    pub(crate) ai_pending_combat_drop_pos: Option<crate::common::Coord3D>,
    pub(crate) ai_fire_has_path: bool,
    pub(crate) ai_fire_waiting_for_path: bool,
    pub(crate) ai_pending_path_goal: Option<Coord3D>,
    pub(crate) ai_pending_ignore_id: Option<ObjectID>,
    pub(crate) ai_pending_path_extra: Option<f32>,
    pub(crate) ai_pending_attack_path: Option<(ObjectID, Coord3D)>,
    pub(crate) ai_pending_original_victim_pos: Option<Option<Coord3D>>,
    pub(crate) ai_pending_clear_victim: bool,
    pub(crate) ai_pending_clear_goal: bool,
    pub(crate) ai_pending_set_victim: Option<ObjectID>,
    pub(crate) ai_pending_path_through_units: Option<bool>,
    pub(crate) ai_pending_allow_invalid_position: Option<bool>,
    pub(crate) ai_pending_goal_id: Option<ObjectID>,
    pub(crate) ai_pending_reset_mood: bool,
    pub(crate) ai_pending_victim_dead: bool,
    pub(crate) ai_pending_destroy_path: bool,
    pub(crate) ai_pending_clear_ignore: bool,
    pub(crate) ai_pending_goal_orientation: Option<f32>,
    pub(crate) ai_pending_goal_position: Option<Coord3D>,
    pub(crate) ai_pending_goal_none: bool,
    pub(crate) ai_pending_turret_objects: Vec<(crate::common::TurretType, Option<ObjectID>, bool)>,
    pub(crate) ai_pending_turret_positions: Vec<(crate::common::TurretType, Coord3D)>,
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

    // Ordered owned behavior views; mutable module state remains in ModuleEntry.
    behaviors: Vec<BehaviorInterfaceHandle>,
    modules: Vec<Arc<ModuleEntry>>,
    // Interface handle lists index into `modules` (C++ keeps m_moduleList plus
    // small friend arrays; entries are never removed individually, so the
    // indices stay stable until the list is released in onDestroy).
    body_module_handles: Vec<usize>,
    die_module_handles: Vec<usize>,
    update_module_handles: Vec<usize>,
    update_module_registrations: Vec<UpdateModulePtr>,
    collide_module_handles: Vec<usize>,
    contain_module_handles: Vec<usize>,
    upgrade_module_handles: Vec<usize>,
    body: Option<Arc<Mutex<dyn BodyModuleInterface>>>,
    contain: Option<Arc<Mutex<dyn ContainModuleInterface>>>,
    stealth: Option<StealthUpdateHandle>,
    ai: Option<Arc<Mutex<dyn AIUpdateInterface>>>,
    physics: Option<Arc<Mutex<dyn PhysicsBehavior>>>,

    // Helper modules
    repulsor_helper: Option<ObjectRepulsorHelper>,
    smc_helper: Option<ObjectSMCHelper>,
    ws_helper: Option<Box<ObjectWeaponStatusHelper>>,
    defection_helper: Option<ObjectDefectionHelper>,
    status_damage_helper: Option<Box<StatusDamageHelper>>,
    subdual_damage_helper: Option<Box<SubdualDamageHelper>>,
    temp_weapon_bonus_helper: Option<Box<TempWeaponBonusHelper>>,
    firing_tracker: Option<Box<FiringTracker>>,
    held_helper: Option<Box<ObjectHeldHelper>>,

    // Spatial and partition data
    partition_data: Option<Box<PartitionData>>,
    radar_data: Option<Box<RadarObject>>,

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
    experience_tracker: Option<Box<ExperienceTracker>>,
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

    // Drawable association. NOT single-owner: this is a shared handle, not the
    // owner. The authoritative owner is the client-side registry
    // (`TheGameClient` `ClientVisualState::drawables`), which also holds
    // objectless drawables (beams, lockon cursors, ropes). Other live borrows
    // of the same `Arc`: `Drawable.attachments`, chinook `RopeInfo::
    // rope_drawable`, garrison `GarrisonPointData::effect`, and the
    // presentation frame, which reads registry entries at render time
    // (`Main/src/presentation_frame/queries.rs`). C++ mirrors this with the
    // Drawable owned by the client DrawableManager and `Object::m_drawable` a
    // raw pointer; `Option<Box<Drawable>>` would orphan objectless drawables.
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

    /// onDestroy and the destructor run at distinct C++ frame boundaries.
    lifecycle: ObjectLifecycle,

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
mod behavior_interfaces;
pub use behavior_interfaces::{
    BehaviorAccessError, BehaviorInterfaceHandle, BehaviorInterfaceLease,
};
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
