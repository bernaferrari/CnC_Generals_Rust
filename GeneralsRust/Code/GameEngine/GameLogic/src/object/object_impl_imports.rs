//! Shared imports for split `impl Object` sibling modules.
//!
//! `use super::*` in children only sees public parent items. This module
//! re-exports the parent crate-imports and private helpers those methods need.

#![allow(unused_imports)]

pub(super) use super::{
    ArmorSetFlag, ArmorSetFlagBits, BehaviorModuleHandle, BehaviorModuleProxy, CrushSquishTestType,
    ModuleEntry, ModuleUpdateProxy, ObjectPrivateStatusBits, ObjectScriptStatusBit, PartitionData,
    RadarObject, SightingInfo, TriggerInfo, UpgradeModuleHandle, armor_set_type_for_flag,
    behavior_downcast_mut, behavior_production_queue_kind, behavior_production_rally_kind,
    behavior_with_downcast, disabled_type_from_index, dual_world_registry_unavailable,
    initial_update_wake_frame, module_behavior_utility_kind, module_die_kind,
    module_dock_update_kind, module_production_behavior_kind, module_production_queue_kind,
    module_upgrade_kind, module_with_downcast, weapon_set_model_condition,
};

pub(super) use once_cell::sync::Lazy;
pub(super) use parking_lot::Mutex as ParkingMutex;
pub(super) use std::any::Any;
pub(super) use std::collections::HashMap;
pub(super) use std::fmt;
pub(super) use std::sync::{Arc, Mutex, RwLock, Weak};

pub(super) use game_engine::common::thing::module_factory::{
    ModuleFactory, get_module_factory, init_module_factory,
};
pub(super) use game_engine::common::{
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
pub(super) use log::warn;

pub(super) use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
pub(super) use crate::common::types::ControlBarInterface;
pub(super) use crate::common::{
    AsciiString, Bool, Byte, Color, CommandSourceType, Coord2D, Coord3D, DefaultThingTemplate,
    Dict, DictType, DisabledMaskType, DisabledType, FormationID, GeometryInfo, ICoord3D, Int,
    KindOf, KindOfMask, KindOfMaskType, LOGICFRAMES_PER_SECOND, Matrix3D, ModelConditionFlags,
    NameKeyType, ObjectID, ObjectShroudStatus, ObjectStatusMaskType, PathfindLayerEnum, PlayerId,
    PlayerMaskType, Real, Relationship, Snapshot, TeamMemberList, Thing, ThingTemplate, TurretType,
    UnsignedByte, UnsignedInt, UpgradeMaskType, VeterancyLevel, WeaponBonusConditionFlags,
};
pub(super) use game_engine::common::game_common::FOREVER;
pub(super) use glam::{EulerRot, Mat4};

pub(super) use crate::ai::HackerAttackMode;
pub(super) use crate::common::xfer::Xfer;
pub(super) use crate::contain_module_overrides::ContainModuleDataKind;

pub(super) use super::CommandSource;
pub(super) use crate::GameLogicResult;
pub(super) use crate::ai::AIGroup;
pub(super) use crate::attack::{ATTACKRESULT_POSSIBLE, AbleToAttackType, CanAttackResult};
pub(super) use crate::common::ArmorSetType;
pub(super) use crate::common::types::WeaponBonusConditionType;
pub(super) use crate::damage::{
    DamageInfo, DamageInfoInput, DamageType, DeathType, HUGE_DAMAGE_AMOUNT,
};
pub(super) use crate::experience::ExperienceTracker;
pub(super) use crate::helpers::{
    FiringTracker, ObjectDisabledHelper, ObjectHeldHelper, TheGameLogic, ThePartitionManager,
};
pub(super) use crate::modules::{
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
pub(super) use crate::object::behavior::flight_deck_behavior::FlightDeckBehaviorModule;
pub(super) use crate::object::behavior::queue_production_exit_behavior::QueueProductionExitBehaviorModule;
pub(super) use crate::object::behavior::special_ability_update::SpecialAbilityUpdate as SpecialAbilityUpdateBehavior;
pub(super) use crate::object::body::body_module::MaxHealthChangeType;
pub(super) use crate::object::die::DieModuleWrapper;
pub(super) use crate::object::drawable::{
    Drawable, DrawableExt, DrawableModuleHandle, DrawableThingHandle,
};
pub(super) use crate::object::helper::{
    ObjectDefectionHelper, ObjectDefectionHelperModuleData, ObjectHelperInterface,
    ObjectRepulsorHelper, ObjectRepulsorHelperModuleData, ObjectSMCHelper,
    ObjectSMCHelperModuleData, ObjectWeaponStatusHelper, StatusDamageHelper, SubdualDamageHelper,
    TempWeaponBonusHelper,
};
pub(super) use crate::object::registry::OBJECT_REGISTRY;
pub(super) use crate::object::special_power_types::{SpecialPowerMask, SpecialPowerType};
pub(super) use crate::object::upgrade::passengers_fire_upgrade::PassengersFireUpgradeHandle;
pub(super) use crate::object::upgrade::status_bits_upgrade::StatusBitsUpgradeHandle;
pub(super) use crate::object::upgrade::subobjects_upgrade::SubObjectsUpgradeHandle;
pub(super) use crate::object_creation_list::nuggets::INVALID_ANGLE;
pub(super) use crate::player::{Player, PlayerIndex, PlayerType, player_list};
pub(super) use crate::scripting::engine::get_event_manager;
pub(super) use crate::scripting::events::{GameEvent, GameEventType};
pub(super) use crate::scripting::{ScriptPriority, ScriptValue};
pub(super) use crate::stealth_update::StealthUpdateHandle;
pub(super) use crate::team::{Team, TeamID};
pub(super) use crate::upgrade::UpgradeTemplate;
pub(super) use crate::upgrade::center::get_upgrade_center;
pub(super) use crate::upgrade_legacy::upgrade_mask_for_ascii;
pub(super) use crate::weapon::{
    Weapon, WeaponAntiMask, WeaponBonusConditionType as WeaponModuleBonusConditionType,
    WeaponChoiceCriteria, WeaponLockType, WeaponSet, WeaponSetFlags, WeaponSetType, WeaponSlotType,
    WeaponStatus,
};
