//! Helper modules for game logic system
//!
//! This module provides helper functionality for various game systems,
//! matching the C++ helper architecture.
//!
//! Split into focused submodules by helper theme.

use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
use crate::common::Matrix3D;
use crate::common::audio::{AudioEventRts, LeftoverAudioOwner, TimeOfDay};
use crate::common::types::{
    EmissionVolumeType, FXListManagerInterface, ParticleSystemManagerInterface,
};
use crate::common::{
    AsciiString, Bool, Color, Coord3D, DISABLED_COUNT, DisabledMaskType, DisabledType,
    DistanceType, FXListId, GeometryInfo, INVALID_ID, Int, KindOf, MessageType, NEVER,
    NameKeyGenerator, NameKeyType, ObjectID, PathfindLayerEnum, PlayerMaskType, Real, Relationship,
    UnsignedInt, VeterancyLevel,
};
use crate::effects::{FXList, ObjectCreationList};
use crate::error::GameLogicError as GameError;
use crate::modules::UpdateModulePtr;
use crate::object::draw::w3d_laser_draw::W3DLaserDrawModuleData;
use crate::object::draw::w3d_tree_draw::W3DTreeDrawModuleData;
use crate::object::drawable::{Drawable, DrawableArcExt, DrawableThingHandle, DrawableType};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::{Object, WEAPONSLOT_COUNT};
use crate::weapon::WeaponBonusSet;
use game_engine::common::audio::audio_event_rts::{
    AudioEventOwnerResolver, register_audio_event_owner_resolver,
};
use game_engine::common::audio::game_audio::{
    AudioLocalityRelationship, AudioLocalityResolver, AudioViewResolver, get_global_audio_manager,
    initialize_global_audio_manager, register_audio_locality_resolver,
    register_audio_view_resolver,
};
use game_engine::common::audio::game_sounds::{
    AudioShroudResolver, register_audio_shroud_resolver,
};
use game_engine::common::audio::{
    AudioAffect as EngineAudioAffect, AudioEventRts as EngineAudioEventRts,
    Coord3D as EngineCoord3D, TimeOfDay as EngineTimeOfDay,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_game_data::ensure_global_data as ensure_engine_global_data;
use game_engine::common::ini::{
    TimeOfDay as IniTimeOfDay, get_global_data as get_engine_global_data,
};
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::radar::get_radar_system;
use game_engine::common::thing::module::{
    ClientUpdateInterface, LaserUpdateInterface, Module, ModuleData, ModuleInterfaceType,
    ModuleType, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::get_module_factory;
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use game_engine::common::thing::thing_template::BuildCompletionType;
use game_engine::common::thing::thing_template::{
    ModuleDescriptorSet, ThingTemplate as EngineThingTemplate,
};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Wave 281: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

#[macro_use]
mod random;
pub use random::*;

include!("lookup.rs");
include!("math.rs");
include!("audio.rs");
include!("object_queries.rs");
include!("game_logic.rs");
include!("particles.rs");
include!("game_client.rs");
include!("object_helpers.rs");
include!("globals.rs");
include!("leftover.rs");
include!("select_object_apply.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const HELPERS_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("audio.rs"),
    include_str!("game_client.rs"),
    include_str!("game_logic.rs"),
    include_str!("globals.rs"),
    include_str!("leftover.rs"),
    include_str!("lookup.rs"),
    include_str!("math.rs"),
    include_str!("object_helpers.rs"),
    include_str!("object_queries.rs"),
    include_str!("particles.rs"),
    include_str!("random.rs"),
);
