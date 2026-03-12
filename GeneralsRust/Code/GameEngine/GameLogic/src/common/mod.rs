//! Common submodules
#![allow(ambiguous_glob_reexports)]

pub mod audio;
pub mod command;
pub mod coord;
pub mod coord_ext;
mod legacy_module;
pub mod option_ext;
pub mod perf_timer;
pub mod result_ext;
pub mod science;
pub mod types;
pub mod vec_ext;
pub mod vector_ext;
pub mod xfer;

pub use crate::error::GameLogicError as GameError;
pub use crate::error::{GameLogicError, GameLogicResult};
pub use legacy_module::LegacyModuleData;

// Re-export types for convenience
pub use coord_ext::*;
pub use option_ext::*;
pub use result_ext::*;
pub use types::*;
pub use vec_ext::*;
pub use vector_ext::*;

#[derive(Debug, Clone, Copy)]
pub enum FieldType {
    UnsignedInt,
    Int,
    String,
    Real,
    Science,
    PercentToReal,
    DurationUnsignedInt,
}

#[derive(Debug, Clone)]
pub struct FieldParse {
    pub token: &'static str,
    pub field_type: FieldType,
    pub target: &'static str,
}

impl FieldParse {
    pub fn new(token: &'static str, field_type: FieldType, target_property: &'static str) -> Self {
        Self {
            token,
            field_type,
            target: target_property,
        }
    }
}

pub use crate::effects::{
    FXList, ObjectCreationList, ParticleSystem, ParticleSystemID, ParticleSystemTemplate,
};
pub use crate::experience::ExperienceTracker;
pub use crate::messages::GameMessage;
pub use crate::object::behavior::behavior_module::BehaviorModuleData;
pub use crate::object::{Object, ObjectTemplate};
pub use crate::player::Player;
pub use crate::team::Team;
pub use crate::terrain::{Bridge, BridgeInfo};
pub use crate::upgrade::{Upgrade, UpgradeStatus, UpgradeTemplate, UpgradeType};
pub use crate::weapon::{WeaponTemplate, WeaponTemplateSet};
pub use game_engine::common::dict::{Dict, DictType, DictValue};
pub use game_engine::common::ini::ini_misc_audio::AudioEventRTS;
pub use game_engine::common::ini::ini_terrain_bridge::TerrainRoadType;
pub use game_engine::common::name_key_generator::NameKeyGenerator;
pub use game_engine::common::thing::module::{ModuleType, NameKeyType, ObjectModule};
pub use game_engine::common::well_known_keys;

/// Generate a name key from a string (convenience wrapper for NameKeyGenerator)
pub fn name_key_generate(name: &str) -> NameKeyType {
    NameKeyGenerator::name_to_key(name)
}

pub use crate::system::game_logic::GameLogic;

pub use crate::helpers::{
    game_logic_random_value, get_game_logic_random_value as GameLogicRandomValue,
    get_game_logic_random_value_real as GameLogicRandomValueReal, TheFXListStore,
    TheGameLODManager, TheInGameUI, TheMessageStream, TheObjectCreationListStore,
    ThePartitionManager, TheThingFactory, TheWeaponStore,
};

pub use crate::common::xfer::XferExt;

pub use crate::helpers::TheThingFactory as ThingFactory;

pub use crate::helpers::TheGameLogic;
pub use crate::helpers::{EvaEvent, TheAudio, TheEva, TheGameText, TheRadar};
pub use crate::system::game_logic::TheObjectFactory;

// Export DamageInfo and DamageTypeFlags from damage module
pub use crate::damage::DamageInfo;
pub use crate::damage::DamageTypeFlags;

// Type aliases for compatibility with C++ naming conventions
/// Thing identifier - used for tracking objects in the game world
/// Matches C++ Thing ID system
pub type ThingId = ObjectID;

/// Object status type alias - matches C++ ObjectStatus usage
/// Use this when referring to individual status flags
pub type ObjectStatus = ObjectStatusTypes;

/// Command source type alias - matches C++ CommandSource usage
/// Use this for indicating where a command originated from
pub type CommandSource = CommandSourceType;

/// Invalid object ID constant - matches C++ INVALID_OBJECT_ID
/// Use this to represent null/invalid object references
pub const INVALID_OBJECT_ID: ObjectID = types::INVALID_ID;

// Re-export command types from commands module
pub use crate::commands::CommandType;

// Re-export weapon slot type from types module
pub use types::WeaponSlotType;
