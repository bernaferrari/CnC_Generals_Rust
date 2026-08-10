//! Common types and utilities shared across all GameLogic modules
//!
//! This module provides type definitions that match the C++ Object system
//! to ensure compatibility and correct behavior.
//!
//! Split into focused submodules by type family.
#![allow(missing_docs)]
#![allow(non_upper_case_globals)]

use crate::physics::PhysicsType;
use bitflags::bitflags;
pub use game_engine::common::ascii_string::AsciiString;
use game_engine::common::bit_flags::ArmorSetBitFlags;
use game_engine::common::global_data;
use game_engine::common::system::object_status_types as legacy_object_status;
use game_engine::common::thing::module::{ModuleData as EngineModuleData, ModuleInterfaceType};
use game_engine::common::thing::thing_template::ModuleDescriptorSet;
use game_engine::system::geometry::{
    GeometryInfo as EngineGeometryInfo, GeometryType as EngineGeometryType,
};
use game_engine::thing::thing_template::{
    ArmorTemplateSet, WeaponTemplateSet as EngineWeaponTemplateSet,
};
use glam::{IVec2, IVec3, Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::{Arc, OnceLock, RwLock};

// Import Object and ThingId for UpdateContext trait methods
use super::ThingId;
use crate::helpers::get_game_logic_random_value_real;
use crate::object::Object;

include!("primitives.rs");
include!("status_masks.rs");
include!("weapon_upgrade_masks.rs");
include!("model_condition.rs");
include!("ids.rs");
include!("enums.rs");
include!("kindof.rs");
include!("geometry.rs");
include!("thing_template.rs");
include!("default_template.rs");
include!("kind_indices.rs");
include!("partition.rs");
include!("update_context.rs");
include!("leftover.rs");
