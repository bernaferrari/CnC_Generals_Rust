//! Common types and utilities shared across all GameLogic modules
//!
//! This module provides type definitions that match the C++ Object system
//! to ensure compatibility and correct behavior.
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

/// Shared result type used across legacy subsystems.
pub type GameResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus, XferVersion};

// Core geometric types matching C++ definitions
/// 3D coordinate type used throughout the game logic system
pub type Coord3D = Vec3;

/// 2D coordinate type used throughout the game logic system  
pub type Coord2D = Vec2;

/// Integer 2D coordinate type  
pub type ICoord2D = IVec2;

/// Integer 3D coordinate type
pub type ICoord3D = IVec3;

/// 3D vector type used for directions and offsets
pub type Vec3D = Vec3;

/// Alias for Vec3D to match C++ usage
pub type Vector3 = Vec3D;

/// Helper trait to provide `origin()` constructors for coordinate aliases.
pub trait CoordOrigin {
    fn origin() -> Self;
}

impl CoordOrigin for Coord3D {
    fn origin() -> Self {
        Vec3::ZERO
    }
}

impl CoordOrigin for Coord2D {
    fn origin() -> Self {
        Vec2::ZERO
    }
}

impl CoordOrigin for ICoord2D {
    fn origin() -> Self {
        IVec2::ZERO
    }
}

impl CoordOrigin for ICoord3D {
    fn origin() -> Self {
        IVec3::ZERO
    }
}

#[derive(Clone)]
pub struct TemplateModuleInfo {
    pub name: AsciiString,
    pub module_tag: AsciiString,
    pub data: Arc<dyn EngineModuleData>,
    pub interface_mask: ModuleInterfaceType,
}

impl TemplateModuleInfo {
    pub fn interface_flags(&self) -> ModuleInterfaceType {
        self.interface_mask
    }
}

/// 3D transformation matrix (SAGE Matrix3D is 4x4 with translation terms)
pub type Matrix3D = Mat4;

/// 4x4 transformation matrix
pub type Matrix4D = Mat4;

/// Real number type (matching C++ Real)
pub type Real = f32;

/// Boolean type (matching C++ Bool)
pub type Bool = bool;

/// Integer type (matching C++ Int)
pub type Int = i32;

/// Unsigned integer type (matching C++ UnsignedInt)
pub type UnsignedInt = u32;

/// Legacy object identifier alias (matching C++ ObjectId)
pub type ObjectId = ObjectID;

/// Unsigned short type (matching C++ UnsignedShort)
pub type UnsignedShort = u16;

/// Short type (matching C++ Short)
pub type Short = i16;

/// Byte type (matching C++ Byte)
pub type Byte = u8;

/// Unsigned byte type (matching C++ UnsignedByte)
pub type UnsignedByte = u8;

// Object identification types
/// Mathematical constants
pub const PI: f32 = std::f32::consts::PI;

/// Timing constants
pub const LOGICFRAMES_PER_SECOND: u32 = 30;
pub const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / LOGICFRAMES_PER_SECOND as f32;

/// Unique identifier for game objects (matching C++ ObjectID)
pub type ObjectID = u32;

/// Player index (matching C++ PlayerIndex)
pub type PlayerIndex = Int;

/// Invalid/null object ID constant
pub const INVALID_ID: ObjectID = 0;

/// Helper trait to enable downcasting from trait objects.
pub trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Module data base trait for all behavior modules
pub trait ModuleData: AsAny + Send + Sync + std::fmt::Debug + std::any::Any {
    /// Returns the canonical module type name (mirrors C++ ModuleDataClass::Get_Module_Name).
    fn get_module_type(&self) -> &str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap_or(full)
    }

    fn get_radar_update_config(
        &self,
    ) -> Option<game_engine::common::thing::module::RadarUpdateConfig> {
        None
    }

    fn get_active_shroud_upgrade_config(
        &self,
    ) -> Option<game_engine::common::thing::module::ActiveShroudUpgradeConfig> {
        None
    }

    fn get_radar_upgrade_config(
        &self,
    ) -> Option<game_engine::common::thing::module::RadarUpgradeConfig> {
        None
    }

    fn get_dynamic_shroud_clearing_range_update_config(
        &self,
    ) -> Option<game_engine::common::thing::module::DynamicShroudClearingRangeUpdateConfig> {
        None
    }
}

impl dyn ModuleData {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

/// Extension trait for Arc<dyn ModuleData> to provide as_any_arc method
pub trait ModuleDataExt {
    fn as_any_arc(self) -> Arc<dyn std::any::Any + Send + Sync>;
}

impl ModuleDataExt for Arc<dyn ModuleData> {
    fn as_any_arc(self) -> Arc<dyn std::any::Any + Send + Sync> {
        // Since ModuleData now extends Any + Send + Sync, we can cast safely
        self as Arc<dyn std::any::Any + Send + Sync>
    }
}

// Game constants
/// Maximum number of players/sides in a game
pub const MAX_PLAYER_COUNT: usize = 8;

/// Maximum number of objects that can exist simultaneously
pub const MAX_OBJECT_COUNT: u32 = 65536;

/// Maximum number of weapon slots
pub const WEAPONSLOT_COUNT: usize = 3;

/// Maximum number of disabled types
pub const DISABLED_COUNT: usize = 13;

/// Maximum trigger area infos
pub const MAX_TRIGGER_AREA_INFOS: usize = 5;

/// Construction complete percentage
pub const CONSTRUCTION_COMPLETE: Real = 100.0;

/// Never timestamp
pub const NEVER: UnsignedInt = 0xFFFFFFFF;

/// Distance calculation mode constants
pub const FROM_CENTER_2D: i32 = 0;
pub const FROM_EDGE_2D: i32 = 1;
pub const FROM_CENTER_3D: i32 = 2;
pub const FROM_BOUNDING_SPHERE_2D: i32 = 3;

/// Distance calculation type
pub type DistanceType = i32;

/// Message type for game messaging system
pub type MessageType = u32;

/// Common message types
pub const MSG_CREATE_SELECTED_GROUP: MessageType = 1001;

/// Frame counter type - represents game simulation frames
pub type FrameNumber = u32;

/// Time in milliseconds
pub type TimeMs = u32;

// Color and rendering types
/// RGBA color type (matching C++ Color)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

unsafe impl Send for Color {}
unsafe impl Sync for Color {}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    pub const fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    pub const fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    /// Convert to packed ARGB (matches C++ Color usage in decals).
    pub const fn to_argb_u32(self) -> u32 {
        ((self.a as u32) << 24) | ((self.b as u32) << 16) | ((self.g as u32) << 8) | (self.r as u32)
    }
}

// Mask types for various object properties
bitflags! {
    /// Object status mask (matching C++ ObjectStatusMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ObjectStatusMaskType: u64 {
        const NONE = 0;
        const DESTROYED = 1u64 << ObjectStatusTypes::Destroyed as u32;
        const CAN_ATTACK = 1u64 << ObjectStatusTypes::CanAttack as u32;
        const UNDER_CONSTRUCTION = 1u64 << ObjectStatusTypes::UnderConstruction as u32;
        const UNSELECTABLE = 1u64 << ObjectStatusTypes::Unselectable as u32;
        const NO_COLLISIONS = 1u64 << ObjectStatusTypes::NoCollisions as u32;
        const NO_ATTACK = 1u64 << ObjectStatusTypes::NoAttack as u32;
        const AIRBORNE_TARGET = 1u64 << ObjectStatusTypes::AirborneTarget as u32;
        const PARACHUTING = 1u64 << ObjectStatusTypes::Parachuting as u32;
        const REPULSOR = 1u64 << ObjectStatusTypes::Repulsor as u32;
        const HIJACKED = 1u64 << ObjectStatusTypes::Hijacked as u32;
        const AFLAME = 1u64 << ObjectStatusTypes::Aflame as u32;
        const BURNED = 1u64 << ObjectStatusTypes::Burned as u32;
        const WET = 1u64 << ObjectStatusTypes::Wet as u32;
        const IS_FIRING_WEAPON = 1u64 << ObjectStatusTypes::IsFiringWeapon as u32;
        const BRAKING = 1u64 << ObjectStatusTypes::Braking as u32;
        const STEALTHED = 1u64 << ObjectStatusTypes::Stealthed as u32;
        const DETECTED = 1u64 << ObjectStatusTypes::Detected as u32;
        const CAN_STEALTH = 1u64 << ObjectStatusTypes::CanStealth as u32;
        const SOLD = 1u64 << ObjectStatusTypes::Sold as u32;
        const UNDERGOING_REPAIR = 1u64 << ObjectStatusTypes::UndergoingRepair as u32;
        const RECONSTRUCTING = 1u64 << ObjectStatusTypes::Reconstructing as u32;
        const MASKED = 1u64 << ObjectStatusTypes::Masked as u32;
        const IS_ATTACKING = 1u64 << ObjectStatusTypes::IsAttacking as u32;
        const IS_USING_ABILITY = 1u64 << ObjectStatusTypes::IsUsingAbility as u32;
        const IS_AIMING_WEAPON = 1u64 << ObjectStatusTypes::IsAimingWeapon as u32;
        const NO_ATTACK_FROM_AI = 1u64 << ObjectStatusTypes::NoAttackFromAi as u32;
        const IGNORING_STEALTH = 1u64 << ObjectStatusTypes::IgnoringStealth as u32;
        const IS_CAR_BOMB = 1u64 << ObjectStatusTypes::IsCarBomb as u32;
        const DECK_HEIGHT_OFFSET = 1u64 << ObjectStatusTypes::DeckHeightOffset as u32;
        const RIDER1 = 1u64 << ObjectStatusTypes::Rider1 as u32;
        const RIDER2 = 1u64 << ObjectStatusTypes::Rider2 as u32;
        const RIDER3 = 1u64 << ObjectStatusTypes::Rider3 as u32;
        const RIDER4 = 1u64 << ObjectStatusTypes::Rider4 as u32;
        const RIDER5 = 1u64 << ObjectStatusTypes::Rider5 as u32;
        const RIDER6 = 1u64 << ObjectStatusTypes::Rider6 as u32;
        const RIDER7 = 1u64 << ObjectStatusTypes::Rider7 as u32;
        const RIDER8 = 1u64 << ObjectStatusTypes::Rider8 as u32;
        const FAERIE_FIRE = 1u64 << ObjectStatusTypes::FaerieFire as u32;
        const MISSILE_KILLING_SELF = 1u64 << ObjectStatusTypes::MissileKillingSelf as u32;
        const REASSIGN_PARKING = 1u64 << ObjectStatusTypes::ReassignParking as u32;
        const BOOBY_TRAPPED = 1u64 << ObjectStatusTypes::BoobyTrapped as u32;
        const IMMOBILE = 1u64 << ObjectStatusTypes::Immobile as u32;
        const DISGUISED = 1u64 << ObjectStatusTypes::Disguised as u32;
        const DEPLOYED = 1u64 << ObjectStatusTypes::Deployed as u32;
    }
}

impl ObjectStatusMaskType {
    /// Empty mask (matches C++ `OBJECT_STATUS_MASK_NONE`)
    pub fn none() -> Self {
        Self::NONE
    }

    /// Create a mask from a single status bit.
    pub const fn from_status(status: ObjectStatusTypes) -> Self {
        match status {
            ObjectStatusTypes::None => Self::NONE,
            _ => Self::from_bits_retain(1u64 << (status as u32)),
        }
    }

    /// Check whether a particular status bit is set.
    pub fn test(&self, status: ObjectStatusTypes) -> bool {
        match status {
            ObjectStatusTypes::None => self.is_empty(),
            _ => self.contains(Self::from_status(status)),
        }
    }

    /// Alias for test() - check whether a particular status bit is set.
    pub fn test_status(&self, status: ObjectStatusTypes) -> bool {
        self.test(status)
    }

    /// Returns true if any status bits are set (mask is not empty).
    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    /// Set a single status bit.
    pub fn set_status(&mut self, status: ObjectStatusTypes) {
        *self |= Self::from_status(status);
    }

    /// Clear a single status bit.
    pub fn clear_status(&mut self, status: ObjectStatusTypes) {
        *self &= !Self::from_status(status);
    }

    /// Parse a list of object-status tokens into a mask, mirroring the legacy helper.
    pub fn parse_tokens<'a, I>(tokens: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let tokens: Vec<&'a str> = tokens.into_iter().collect();
        let has_none = tokens
            .iter()
            .any(|token| token.eq_ignore_ascii_case("NONE"));
        if has_none && tokens.len() > 1 {
            return Err("mixing NONE with other tokens is invalid".to_string());
        }

        let legacy_mask =
            legacy_object_status::ObjectStatusMaskType::parse_tokens(tokens.iter().copied())?;
        Ok(Self::from_bits_retain(legacy_mask.bits()))
    }

    pub fn from_case_insensitive_name(name: &str) -> Option<Self> {
        Self::parse_tokens(std::iter::once(name)).ok()
    }
}

/// Implement From trait to convert ObjectStatusTypes to ObjectStatusMaskType
impl From<ObjectStatusTypes> for ObjectStatusMaskType {
    fn from(status: ObjectStatusTypes) -> Self {
        Self::from_status(status)
    }
}

bitflags! {
    /// Special power mask (matching C++ SpecialPowerMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpecialPowerMaskType: u32 {
        const SUPERWEAPON_A = 1 << 0;
        const SUPERWEAPON_B = 1 << 1;
        const SUPERWEAPON_C = 1 << 2;
        const CASH_HACK = 1 << 3;
        const RADAR_VAN_SCAN = 1 << 4;
        const SPY_SATELLITE = 1 << 5;
        const DISGUISE = 1 << 6;
        const RADAR_JAMMER = 1 << 7;
        // Add more as needed
    }
}

#[cfg(test)]
mod tests {
    use super::{
        engine_geometry_to_logic, geometry_type_from_u32, geometry_type_to_u32, EngineGeometryInfo,
        EngineGeometryType, GeometryExtentModType, GeometryInfo, ObjectStatusMaskType,
    };

    #[test]
    fn object_status_parse_tokens_matches_legacy_helper() {
        let mask = ObjectStatusMaskType::parse_tokens(["STEALTHED", "DETECTED"].iter().copied())
            .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::STEALTHED));
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::AFLAME));
    }

    #[test]
    fn object_status_parse_tokens_accepts_additive_modifiers() {
        let mask = ObjectStatusMaskType::parse_tokens(
            ["+STEALTHED", "+DETECTED", "-STEALTHED"].iter().copied(),
        )
        .expect("parse succeeds");
        assert!(mask.contains(ObjectStatusMaskType::DETECTED));
        assert!(!mask.contains(ObjectStatusMaskType::STEALTHED));
    }

    #[test]
    fn object_status_parse_tokens_errors_on_mixed_none() {
        let err = ObjectStatusMaskType::parse_tokens(["NONE", "STEALTHED"].iter().copied())
            .expect_err("mixing NONE with other tokens is invalid");
        assert!(
            err.contains("NONE"),
            "error message should reference NONE token"
        );
    }

    #[test]
    fn geometry_info_tweak_extents_cycles_type_and_clears_small_flag() {
        let mut geometry = GeometryInfo::default();
        geometry.geometry_type = EngineGeometryType::Box;
        geometry.is_small = true;
        geometry.tweak_extents(GeometryExtentModType::Type, 1.0);

        assert_eq!(geometry.geometry_type, EngineGeometryType::Sphere);
        assert!(!geometry.is_small);
    }

    #[test]
    fn geometry_type_round_trip_helpers_match_cpp_order() {
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Sphere), 0);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Cylinder), 1);
        assert_eq!(geometry_type_to_u32(EngineGeometryType::Box), 2);

        assert_eq!(geometry_type_from_u32(0), EngineGeometryType::Sphere);
        assert_eq!(geometry_type_from_u32(1), EngineGeometryType::Cylinder);
        assert_eq!(geometry_type_from_u32(2), EngineGeometryType::Box);
    }

    #[test]
    fn engine_geometry_to_logic_preserves_type_and_small_flag() {
        let engine_geometry =
            EngineGeometryInfo::new(EngineGeometryType::Cylinder, true, 12.0, 8.0, 4.0);

        let logic_geometry = engine_geometry_to_logic(&engine_geometry);

        assert_eq!(logic_geometry.geometry_type, EngineGeometryType::Cylinder);
        assert!(logic_geometry.is_small);
        assert_eq!(logic_geometry.get_major_radius(), 6.0);
        assert_eq!(logic_geometry.get_minor_radius(), 2.0);
        assert_eq!(logic_geometry.get_max_height_above_position(), 8.0);
    }
}

impl SpecialPowerMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

bitflags! {
    /// Disabled mask (matching C++ DisabledMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisabledMaskType: u32 {
        const DISABLED_DEFAULT = 1 << 0;
        const DISABLED_HACKED = 1 << 1;
        const DISABLED_EMP = 1 << 2;
        const HELD = 1 << 3;
        const PARALYZED = 1 << 4;
        const DISABLED_UNMANNED = 1 << 5;
        const DISABLED_UNDERPOWERED = 1 << 6;
        const DISABLED_FREEFALL = 1 << 7;
        const DISABLED_AWESTRUCK = 1 << 8;
        const DISABLED_BRAINWASHED = 1 << 9;
        const DISABLED_SUBDUED = 1 << 10;
        const DISABLED_SCRIPT_DISABLED = 1 << 11;
        const DISABLED_SCRIPT_UNDERPOWERED = 1 << 12;
    }
}

impl DisabledMaskType {
    pub fn none() -> Self {
        Self::empty()
    }

    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    pub fn test(&self, disabled_type: DisabledType) -> bool {
        match disabled_type {
            DisabledType::DisabledDefault => self.contains(Self::DISABLED_DEFAULT),
            DisabledType::DisabledHacked => self.contains(Self::DISABLED_HACKED),
            DisabledType::DisabledEmp => self.contains(Self::DISABLED_EMP),
            DisabledType::Held => self.contains(Self::HELD),
            DisabledType::Paralyzed => self.contains(Self::PARALYZED),
            DisabledType::DisabledSubdued => self.contains(Self::DISABLED_SUBDUED),
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                self.contains(Self::DISABLED_UNMANNED)
            }
            DisabledType::DisabledUnderpowered => self.contains(Self::DISABLED_UNDERPOWERED),
            DisabledType::DisabledFreefall => self.contains(Self::DISABLED_FREEFALL),
            DisabledType::DisabledAwestruck => self.contains(Self::DISABLED_AWESTRUCK),
            DisabledType::DisabledBrainwashed => self.contains(Self::DISABLED_BRAINWASHED),
            DisabledType::DisabledScriptDisabled => self.contains(Self::DISABLED_SCRIPT_DISABLED),
            DisabledType::DisabledScriptUnderpowered => {
                self.contains(Self::DISABLED_SCRIPT_UNDERPOWERED)
            }
            DisabledType::DisabledAny => self.any(),
        }
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self |= Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self |= Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self |= Self::DISABLED_EMP,
            DisabledType::Held => *self |= Self::HELD,
            DisabledType::Paralyzed => *self |= Self::PARALYZED,
            DisabledType::DisabledSubdued => *self |= Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self |= Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self |= Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self |= Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self |= Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self |= Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self |= Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => *self |= Self::DISABLED_SCRIPT_UNDERPOWERED,
            DisabledType::DisabledAny => {} // No-op for aggregated state
        }
    }

    pub fn clear(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self &= !Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self &= !Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self &= !Self::DISABLED_EMP,
            DisabledType::Held => *self &= !Self::HELD,
            DisabledType::Paralyzed => *self &= !Self::PARALYZED,
            DisabledType::DisabledSubdued => *self &= !Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self &= !Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self &= !Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self &= !Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self &= !Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self &= !Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self &= !Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => {
                *self &= !Self::DISABLED_SCRIPT_UNDERPOWERED
            }
            DisabledType::DisabledAny => *self = Self::empty(),
        }
    }
}

/// Type alias for backward compatibility with C++ naming
pub type DisabledMask = DisabledMaskType;

/// ID type for ThingTemplates
pub type ThingTemplateId = u32;

/// ID type for UpgradeTemplates
pub type UpgradeTemplateId = u32;

/// Production ID for tracking unit construction
pub type ProductionID = u32;

/// Invalid production ID constant
pub const PRODUCTIONID_INVALID: ProductionID = 0;

bitflags! {
    /// Weapon bonus condition flags (matching C++ WeaponBonusConditionFlags)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct WeaponBonusConditionFlags: u32 {
        const GARRISONED = 1 << 0;
        const HORDE = 1 << 1;
        const CONTINUOUS_FIRE_MEAN = 1 << 2;
        const CONTINUOUS_FIRE_FAST = 1 << 3;
        const NATIONALISM = 1 << 4;
        const PLAYER_UPGRADE = 1 << 5;
        const DRONE_SPOTTING = 1 << 6;
        const DEMORALIZED = 1 << 7;
        const DEMORALIZED_OBSOLETE = 1 << 8;
        const ENTHUSIASTIC = 1 << 9;
        const VETERAN = 1 << 10;
        const ELITE = 1 << 11;
        const HERO = 1 << 12;
        const BATTLEPLAN_BOMBARDMENT = 1 << 13;
        const BATTLEPLAN_HOLDTHELINE = 1 << 14;
        const BATTLEPLAN_SEARCHANDDESTROY = 1 << 15;
        const SUBLIMINAL = 1 << 16;
        const SOLO_HUMAN_EASY = 1 << 17;
        const SOLO_HUMAN_NORMAL = 1 << 18;
        const SOLO_HUMAN_HARD = 1 << 19;
        const SOLO_AI_EASY = 1 << 20;
        const SOLO_AI_NORMAL = 1 << 21;
        const SOLO_AI_HARD = 1 << 22;
        const TARGET_FAERIE_FIRE = 1 << 23;
        const FANATICISM = 1 << 24;
        const FRENZY_ONE = 1 << 25;
        const FRENZY_TWO = 1 << 26;
        const FRENZY_THREE = 1 << 27;
        const DRONE_SPOT_FOR_STRIKE = 1 << 28;
    }
}

impl WeaponBonusConditionFlags {
    pub fn new() -> Self {
        Self::none()
    }

    pub fn none() -> Self {
        Self::from_bits_truncate(0)
    }

    /// Clear specific condition flag(s) from the mask
    pub fn clear(&mut self, condition: WeaponBonusConditionType) {
        // Convert WeaponBonusConditionType to the appropriate flag and remove it
        let flag = match condition {
            WeaponBonusConditionType::Invalid => return,
            WeaponBonusConditionType::Garrisoned => Self::GARRISONED,
            WeaponBonusConditionType::Horde => Self::HORDE,
            WeaponBonusConditionType::ContinuousFireMean => Self::CONTINUOUS_FIRE_MEAN,
            WeaponBonusConditionType::ContinuousFireFast => Self::CONTINUOUS_FIRE_FAST,
            WeaponBonusConditionType::Nationalism => Self::NATIONALISM,
            WeaponBonusConditionType::PlayerUpgrade => Self::PLAYER_UPGRADE,
            WeaponBonusConditionType::DroneSpotting => Self::DRONE_SPOTTING,
            WeaponBonusConditionType::Demoralized => Self::DEMORALIZED,
            WeaponBonusConditionType::Elite => Self::ELITE,
            WeaponBonusConditionType::Veteran => Self::VETERAN,
            WeaponBonusConditionType::DroneSpotForStrike => Self::DRONE_SPOT_FOR_STRIKE,
            WeaponBonusConditionType::DemoralizedObsolete => Self::DEMORALIZED_OBSOLETE,
            WeaponBonusConditionType::Enthusiastic => Self::ENTHUSIASTIC,
            WeaponBonusConditionType::Hero => Self::HERO,
            WeaponBonusConditionType::BattlePlanBombardment => Self::BATTLEPLAN_BOMBARDMENT,
            WeaponBonusConditionType::BattlePlanHoldTheLine => Self::BATTLEPLAN_HOLDTHELINE,
            WeaponBonusConditionType::BattlePlanSearchAndDestroy => {
                Self::BATTLEPLAN_SEARCHANDDESTROY
            }
            WeaponBonusConditionType::Subliminal => Self::SUBLIMINAL,
            WeaponBonusConditionType::SoloHumanEasy => Self::SOLO_HUMAN_EASY,
            WeaponBonusConditionType::SoloHumanNormal => Self::SOLO_HUMAN_NORMAL,
            WeaponBonusConditionType::SoloHumanHard => Self::SOLO_HUMAN_HARD,
            WeaponBonusConditionType::SoloAiEasy => Self::SOLO_AI_EASY,
            WeaponBonusConditionType::SoloAiNormal => Self::SOLO_AI_NORMAL,
            WeaponBonusConditionType::SoloAiHard => Self::SOLO_AI_HARD,
            WeaponBonusConditionType::TargetFaerieFire => Self::TARGET_FAERIE_FIRE,
            WeaponBonusConditionType::Fanaticism => Self::FANATICISM,
            WeaponBonusConditionType::FrenzyOne => Self::FRENZY_ONE,
            WeaponBonusConditionType::FrenzyTwo => Self::FRENZY_TWO,
            WeaponBonusConditionType::FrenzyThree => Self::FRENZY_THREE,
        };
        self.remove(flag);
    }

    /// Set a specific condition flag in the mask
    pub fn set_condition(&mut self, condition: WeaponBonusConditionType) {
        let flag = match condition {
            WeaponBonusConditionType::Invalid => return,
            WeaponBonusConditionType::Garrisoned => Self::GARRISONED,
            WeaponBonusConditionType::Horde => Self::HORDE,
            WeaponBonusConditionType::ContinuousFireMean => Self::CONTINUOUS_FIRE_MEAN,
            WeaponBonusConditionType::ContinuousFireFast => Self::CONTINUOUS_FIRE_FAST,
            WeaponBonusConditionType::Nationalism => Self::NATIONALISM,
            WeaponBonusConditionType::PlayerUpgrade => Self::PLAYER_UPGRADE,
            WeaponBonusConditionType::DroneSpotting => Self::DRONE_SPOTTING,
            WeaponBonusConditionType::Demoralized => Self::DEMORALIZED,
            WeaponBonusConditionType::Elite => Self::ELITE,
            WeaponBonusConditionType::Veteran => Self::VETERAN,
            WeaponBonusConditionType::DroneSpotForStrike => Self::DRONE_SPOT_FOR_STRIKE,
            WeaponBonusConditionType::DemoralizedObsolete => Self::DEMORALIZED_OBSOLETE,
            WeaponBonusConditionType::Enthusiastic => Self::ENTHUSIASTIC,
            WeaponBonusConditionType::Hero => Self::HERO,
            WeaponBonusConditionType::BattlePlanBombardment => Self::BATTLEPLAN_BOMBARDMENT,
            WeaponBonusConditionType::BattlePlanHoldTheLine => Self::BATTLEPLAN_HOLDTHELINE,
            WeaponBonusConditionType::BattlePlanSearchAndDestroy => {
                Self::BATTLEPLAN_SEARCHANDDESTROY
            }
            WeaponBonusConditionType::Subliminal => Self::SUBLIMINAL,
            WeaponBonusConditionType::SoloHumanEasy => Self::SOLO_HUMAN_EASY,
            WeaponBonusConditionType::SoloHumanNormal => Self::SOLO_HUMAN_NORMAL,
            WeaponBonusConditionType::SoloHumanHard => Self::SOLO_HUMAN_HARD,
            WeaponBonusConditionType::SoloAiEasy => Self::SOLO_AI_EASY,
            WeaponBonusConditionType::SoloAiNormal => Self::SOLO_AI_NORMAL,
            WeaponBonusConditionType::SoloAiHard => Self::SOLO_AI_HARD,
            WeaponBonusConditionType::TargetFaerieFire => Self::TARGET_FAERIE_FIRE,
            WeaponBonusConditionType::Fanaticism => Self::FANATICISM,
            WeaponBonusConditionType::FrenzyOne => Self::FRENZY_ONE,
            WeaponBonusConditionType::FrenzyTwo => Self::FRENZY_TWO,
            WeaponBonusConditionType::FrenzyThree => Self::FRENZY_THREE,
        };
        self.insert(flag);
    }
}

bitflags! {
    /// Weapon set flags (matching C++ WeaponSetFlags)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WeaponSetFlags: u32 {
        const PRIMARY_WEAPON = 1 << 0;
        const SECONDARY_WEAPON = 1 << 1;
        const TERTIARY_WEAPON = 1 << 2;
        const PASSENGER_WEAPON = 1 << 3;
        const PLAYER_UPGRADE = 1 << 4;
        const VETERAN = 1 << 5;
        // Add more as needed
    }
}

impl WeaponSetFlags {
    pub fn new() -> Self {
        Self::empty()
    }

    pub fn test(&self, weapon_set_type: WeaponSetType) -> bool {
        match weapon_set_type {
            WeaponSetType::Primary => self.contains(Self::PRIMARY_WEAPON),
            WeaponSetType::Secondary => self.contains(Self::SECONDARY_WEAPON),
            WeaponSetType::Tertiary => self.contains(Self::TERTIARY_WEAPON),
            WeaponSetType::Passenger => self.contains(Self::PASSENGER_WEAPON),
        }
    }
}

bitflags! {
    /// Upgrade mask (matching C++ UpgradeMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct UpgradeMaskType: u128 {
        // Define upgrade bits as needed
    }
}

impl UpgradeMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

bitflags! {
    /// Player mask (matching C++ PlayerMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlayerMaskType: u32 {
        const PLAYER_1 = 1 << 0;
        const PLAYER_2 = 1 << 1;
        const PLAYER_3 = 1 << 2;
        const PLAYER_4 = 1 << 3;
        const PLAYER_5 = 1 << 4;
        const PLAYER_6 = 1 << 5;
        const PLAYER_7 = 1 << 6;
        const PLAYER_8 = 1 << 7;
    }
}

impl PlayerMaskType {
    pub fn none() -> Self {
        Self::empty()
    }
}

/// All players mask (matching C++ PLAYERMASK_ALL = 0xffff)
pub const PLAYERMASK_ALL: PlayerMaskType = PlayerMaskType::all();

bitflags! {
    /// Model condition flags — bit positions match C++ ModelState enum exactly.
    /// Bit N corresponds to C++ ModelConditionType enum value N.
    /// Authoritative source: Common/src/common/bit_flags.rs
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ModelConditionFlags: u128 {
        // --- C++ ModelConditionType enum values 0-117 (authoritative) ---
        // Bit 0: TOPPLED
        const TOPPLED = 1 << 0;
        // Bit 1: FRONTCRUSHED
        const FRONTCRUSHED = 1 << 1;
        // Bit 2: BACKCRUSHED
        const BACKCRUSHED = 1 << 2;
        // Bit 3: DAMAGED
        const DAMAGED = 1 << 3;
        // Bit 4: REALLYDAMAGED
        const REALLYDAMAGED = 1 << 4;
        // Bit 5: RUBBLE
        const RUBBLE = 1 << 5;
        // Bit 6: SPECIAL_DAMAGED
        const SPECIAL_DAMAGED = 1 << 6;
        // Bit 7: NIGHT
        const NIGHT = 1 << 7;
        // Bit 8: SNOW
        const SNOW = 1 << 8;
        // Bit 9: PARACHUTING
        const PARACHUTING = 1 << 9;
        // Bit 10: GARRISONED
        const GARRISONED = 1 << 10;
        // Bit 11: ENEMYNEAR
        const ENEMYNEAR = 1 << 11;
        // Bit 12: WEAPONSET_VETERAN
        const WEAPONSET_VETERAN = 1 << 12;
        // Bit 13: WEAPONSET_ELITE
        const WEAPONSET_ELITE = 1 << 13;
        // Bit 14: WEAPONSET_HERO
        const WEAPONSET_HERO = 1 << 14;
        // Bit 15: WEAPONSET_CRATEUPGRADE_ONE
        const WEAPONSET_CRATEUPGRADE_ONE = 1 << 15;
        // Bit 16: WEAPONSET_CRATEUPGRADE_TWO
        const WEAPONSET_CRATEUPGRADE_TWO = 1 << 16;
        // Bit 17: WEAPONSET_PLAYER_UPGRADE
        const WEAPONSET_PLAYER_UPGRADE = 1 << 17;
        // Bit 18: DOOR_1_OPENING
        const DOOR_1_OPENING = 1 << 18;
        // Bit 19: DOOR_1_CLOSING
        const DOOR_1_CLOSING = 1 << 19;
        // Bit 20: DOOR_1_WAITING_OPEN
        const DOOR_1_WAITING_OPEN = 1 << 20;
        // Bit 21: DOOR_1_WAITING_TO_CLOSE
        const DOOR_1_WAITING_TO_CLOSE = 1 << 21;
        // Bit 22: DOOR_2_OPENING
        const DOOR_2_OPENING = 1 << 22;
        // Bit 23: DOOR_2_CLOSING
        const DOOR_2_CLOSING = 1 << 23;
        // Bit 24: DOOR_2_WAITING_OPEN
        const DOOR_2_WAITING_OPEN = 1 << 24;
        // Bit 25: DOOR_2_WAITING_TO_CLOSE
        const DOOR_2_WAITING_TO_CLOSE = 1 << 25;
        // Bit 26: DOOR_3_OPENING
        const DOOR_3_OPENING = 1 << 26;
        // Bit 27: DOOR_3_CLOSING
        const DOOR_3_CLOSING = 1 << 27;
        // Bit 28: DOOR_3_WAITING_OPEN
        const DOOR_3_WAITING_OPEN = 1 << 28;
        // Bit 29: DOOR_3_WAITING_TO_CLOSE
        const DOOR_3_WAITING_TO_CLOSE = 1 << 29;
        // Bit 30: DOOR_4_OPENING
        const DOOR_4_OPENING = 1 << 30;
        // Bit 31: DOOR_4_CLOSING
        const DOOR_4_CLOSING = 1u128 << 31;
        // Bit 32: DOOR_4_WAITING_OPEN
        const DOOR_4_WAITING_OPEN = 1u128 << 32;
        // Bit 33: DOOR_4_WAITING_TO_CLOSE
        const DOOR_4_WAITING_TO_CLOSE = 1u128 << 33;
        // Bit 34: ATTACKING
        const ATTACKING = 1u128 << 34;
        // Bit 35: PREATTACK_A
        const PREATTACK_A = 1u128 << 35;
        // Bit 36: FIRING_A
        const FIRING_A = 1u128 << 36;
        // Bit 37: BETWEEN_FIRING_SHOTS_A
        const BETWEEN_FIRING_SHOTS_A = 1u128 << 37;
        // Bit 38: RELOADING_A
        const RELOADING_A = 1u128 << 38;
        // Bit 39: PREATTACK_B
        const PREATTACK_B = 1u128 << 39;
        // Bit 40: FIRING_B
        const FIRING_B = 1u128 << 40;
        // Bit 41: BETWEEN_FIRING_SHOTS_B
        const BETWEEN_FIRING_SHOTS_B = 1u128 << 41;
        // Bit 42: RELOADING_B
        const RELOADING_B = 1u128 << 42;
        // Bit 43: PREATTACK_C
        const PREATTACK_C = 1u128 << 43;
        // Bit 44: FIRING_C
        const FIRING_C = 1u128 << 44;
        // Bit 45: BETWEEN_FIRING_SHOTS_C
        const BETWEEN_FIRING_SHOTS_C = 1u128 << 45;
        // Bit 46: RELOADING_C
        const RELOADING_C = 1u128 << 46;
        // Bit 47: TURRET_ROTATE
        const TURRET_ROTATE = 1u128 << 47;
        // Bit 48: POST_COLLAPSE
        const POST_COLLAPSE = 1u128 << 48;
        // Bit 49: MOVING
        const MOVING = 1u128 << 49;
        // Bit 50: DYING
        const DYING = 1u128 << 50;
        // Bit 51: AWAITING_CONSTRUCTION
        const AWAITING_CONSTRUCTION = 1u128 << 51;
        // Bit 52: PARTIALLY_CONSTRUCTED
        const PARTIALLY_CONSTRUCTED = 1u128 << 52;
        // Bit 53: ACTIVELY_BEING_CONSTRUCTED
        const ACTIVELY_BEING_CONSTRUCTED = 1u128 << 53;
        // Bit 54: PRONE
        const PRONE = 1u128 << 54;
        // Bit 55: FREEFALL
        const FREEFALL = 1u128 << 55;
        // Bit 56: ACTIVELY_CONSTRUCTING
        const ACTIVELY_CONSTRUCTING = 1u128 << 56;
        // Bit 57: CONSTRUCTION_COMPLETE
        const CONSTRUCTION_COMPLETE = 1u128 << 57;
        // Bit 58: RADAR_EXTENDING
        const RADAR_EXTENDING = 1u128 << 58;
        // Bit 59: RADAR_UPGRADED
        const RADAR_UPGRADED = 1u128 << 59;
        // Bit 60: PANICKING
        const PANICKING = 1u128 << 60;
        // Bit 61: AFLAME
        const AFLAME = 1u128 << 61;
        // Bit 62: SMOLDERING
        const SMOLDERING = 1u128 << 62;
        // Bit 63: BURNED
        const BURNED = 1u128 << 63;
        // Bit 64: DOCKING
        const DOCKING = 1u128 << 64;
        // Bit 65: DOCKING_BEGINNING
        const DOCKING_BEGINNING = 1u128 << 65;
        // Bit 66: DOCKING_ACTIVE
        const DOCKING_ACTIVE = 1u128 << 66;
        // Bit 67: DOCKING_ENDING
        const DOCKING_ENDING = 1u128 << 67;
        // Bit 68: CARRYING
        const CARRYING = 1u128 << 68;
        // Bit 69: FLOODED
        const FLOODED = 1u128 << 69;
        // Bit 70: LOADED
        const LOADED = 1u128 << 70;
        // Bit 71: JETAFTERBURNER
        const JETAFTERBURNER = 1u128 << 71;
        // Bit 72: JETEXHAUST
        const JETEXHAUST = 1u128 << 72;
        // Bit 73: PACKING
        const PACKING = 1u128 << 73;
        // Bit 74: UNPACKING
        const UNPACKING = 1u128 << 74;
        // Bit 75: DEPLOYED
        const DEPLOYED = 1u128 << 75;
        // Bit 76: OVER_WATER
        const OVER_WATER = 1u128 << 76;
        // Bit 77: POWER_PLANT_UPGRADED
        const POWER_PLANT_UPGRADED = 1u128 << 77;
        // Bit 78: CLIMBING
        const CLIMBING = 1u128 << 78;
        // Bit 79: SOLD
        const SOLD = 1u128 << 79;
        // Bit 80: RAPPELLING
        const RAPPELLING = 1u128 << 80;
        // Bit 81: ARMED
        const ARMED = 1u128 << 81;
        // Bit 82: POWER_PLANT_UPGRADING
        const POWER_PLANT_UPGRADING = 1u128 << 82;
        // Bit 83: SPECIAL_CHEERING
        const SPECIAL_CHEERING = 1u128 << 83;
        // Bit 84: CONTINUOUS_FIRE_SLOW
        const CONTINUOUS_FIRE_SLOW = 1u128 << 84;
        // Bit 85: CONTINUOUS_FIRE_MEAN
        const CONTINUOUS_FIRE_MEAN = 1u128 << 85;
        // Bit 86: CONTINUOUS_FIRE_FAST
        const CONTINUOUS_FIRE_FAST = 1u128 << 86;
        // Bit 87: RAISING_FLAG
        const RAISING_FLAG = 1u128 << 87;
        // Bit 88: CAPTURED
        const CAPTURED = 1u128 << 88;
        // Bit 89: EXPLODED_FLAILING
        const EXPLODED_FLAILING = 1u128 << 89;
        // Bit 90: EXPLODED_BOUNCING
        const EXPLODED_BOUNCING = 1u128 << 90;
        // Bit 91: SPLATTED
        const SPLATTED = 1u128 << 91;
        // Bit 92: USING_WEAPON_A
        const USING_WEAPON_A = 1u128 << 92;
        // Bit 93: USING_WEAPON_B
        const USING_WEAPON_B = 1u128 << 93;
        // Bit 94: USING_WEAPON_C
        const USING_WEAPON_C = 1u128 << 94;
        // Bit 95: PREORDER
        const PREORDER = 1u128 << 95;
        // Bit 96: CENTER_TO_LEFT
        const CENTER_TO_LEFT = 1u128 << 96;
        // Bit 97: LEFT_TO_CENTER
        const LEFT_TO_CENTER = 1u128 << 97;
        // Bit 98: CENTER_TO_RIGHT
        const CENTER_TO_RIGHT = 1u128 << 98;
        // Bit 99: RIGHT_TO_CENTER
        const RIGHT_TO_CENTER = 1u128 << 99;
        // Bit 100: RIDER1
        const RIDER1 = 1u128 << 100;
        // Bit 101: RIDER2
        const RIDER2 = 1u128 << 101;
        // Bit 102: RIDER3
        const RIDER3 = 1u128 << 102;
        // Bit 103: RIDER4
        const RIDER4 = 1u128 << 103;
        // Bit 104: RIDER5
        const RIDER5 = 1u128 << 104;
        // Bit 105: RIDER6
        const RIDER6 = 1u128 << 105;
        // Bit 106: RIDER7
        const RIDER7 = 1u128 << 106;
        // Bit 107: RIDER8
        const RIDER8 = 1u128 << 107;
        // Bit 108: STUNNED_FLAILING
        const STUNNED_FLAILING = 1u128 << 108;
        // Bit 109: STUNNED
        const STUNNED = 1u128 << 109;
        // Bit 110: SECOND_LIFE
        const SECOND_LIFE = 1u128 << 110;
        // Bit 111: JAMMED
        const JAMMED = 1u128 << 111;
        // Bit 112: ARMORSET_CRATEUPGRADE_ONE
        const ARMORSET_CRATEUPGRADE_ONE = 1u128 << 112;
        // Bit 113: ARMORSET_CRATEUPGRADE_TWO
        const ARMORSET_CRATEUPGRADE_TWO = 1u128 << 113;
        // Bit 114: USER_1
        const USER_1 = 1u128 << 114;
        // Bit 115: USER_2
        const USER_2 = 1u128 << 115;
        // Bit 116: (reserved)
        // Bit 117: DISGUISED
        const DISGUISED = 1u128 << 117;

        // --- PascalCase / compatibility aliases (same bits as above) ---
        // Door aliases (PascalCase)
        const Door1Opening = Self::DOOR_1_OPENING.bits();
        const Door1WaitingOpen = Self::DOOR_1_WAITING_OPEN.bits();
        const Door1Closing = Self::DOOR_1_CLOSING.bits();
        const Door1WaitingToClose = Self::DOOR_1_WAITING_TO_CLOSE.bits();
        const Door2Opening = Self::DOOR_2_OPENING.bits();
        const Door2WaitingOpen = Self::DOOR_2_WAITING_OPEN.bits();
        const Door2Closing = Self::DOOR_2_CLOSING.bits();
        const Door2WaitingToClose = Self::DOOR_2_WAITING_TO_CLOSE.bits();
        const Door3Opening = Self::DOOR_3_OPENING.bits();
        const Door3WaitingOpen = Self::DOOR_3_WAITING_OPEN.bits();
        const Door3Closing = Self::DOOR_3_CLOSING.bits();
        const Door3WaitingToClose = Self::DOOR_3_WAITING_TO_CLOSE.bits();
        const Door4Opening = Self::DOOR_4_OPENING.bits();
        const Door4WaitingOpen = Self::DOOR_4_WAITING_OPEN.bits();
        const Door4Closing = Self::DOOR_4_CLOSING.bits();
        const Door4WaitingToClose = Self::DOOR_4_WAITING_TO_CLOSE.bits();
        // Steering aliases (PascalCase, no underscores)
        const CenterToRight = Self::CENTER_TO_RIGHT.bits();
        const CenterToLeft = Self::CENTER_TO_LEFT.bits();
        const RightToCenter = Self::RIGHT_TO_CENTER.bits();
        const LeftToCenter = Self::LEFT_TO_CENTER.bits();
        // Packing/unpacking aliases (PascalCase)
        const Packing = Self::PACKING.bits();
        const Unpacking = Self::UNPACKING.bits();
        // Weapon fire state aliases (PascalCase)
        const FiringA = Self::FIRING_A.bits();
        const FiringB = Self::FIRING_B.bits();
        const FiringC = Self::FIRING_C.bits();
        const BetweenFiringShotsA = Self::BETWEEN_FIRING_SHOTS_A.bits();
        const BetweenFiringShotsB = Self::BETWEEN_FIRING_SHOTS_B.bits();
        const BetweenFiringShotsC = Self::BETWEEN_FIRING_SHOTS_C.bits();
        const ReloadingA = Self::RELOADING_A.bits();
        const ReloadingB = Self::RELOADING_B.bits();
        const ReloadingC = Self::RELOADING_C.bits();
        const PreAttackA = Self::PREATTACK_A.bits();
        const PreAttackB = Self::PREATTACK_B.bits();
        const PreAttackC = Self::PREATTACK_C.bits();
        const UsingWeaponA = Self::USING_WEAPON_A.bits();
        const UsingWeaponB = Self::USING_WEAPON_B.bits();
        const UsingWeaponC = Self::USING_WEAPON_C.bits();
        // Construction aliases (PascalCase)
        const ActivelyConstructing = Self::ACTIVELY_CONSTRUCTING.bits();
        const ConstructionComplete = Self::CONSTRUCTION_COMPLETE.bits();
        // Radar aliases (PascalCase)
        const RadarExtending = Self::RADAR_EXTENDING.bits();
        const RadarUpgraded = Self::RADAR_UPGRADED.bits();
        // PowerPlant aliases (PascalCase)
        const PowerPlantUpgrading = Self::POWER_PLANT_UPGRADING.bits();
        const PowerPlantUpgraded = Self::POWER_PLANT_UPGRADED.bits();
        // Flame aliases (PascalCase — now canonical since bit positions are correct)
        const Aflame = Self::AFLAME.bits();
        const Smoldering = Self::SMOLDERING.bits();
        const Burned = Self::BURNED.bits();
        // Armorset aliases (PascalCase)
        const ArmorsetCrateUpgradeOne = Self::ARMORSET_CRATEUPGRADE_ONE.bits();
        const ArmorsetCrateUpgradeTwo = Self::ARMORSET_CRATEUPGRADE_TWO.bits();
        const Loaded = Self::LOADED.bits();

        // --- Extra names not in C++ ModelConditionType but used by GameLogic code ---
        // These occupy bits beyond 117 to avoid collisions with C++ enum values.
        // Invalid = empty (no condition)
        const Invalid = 0;
        // PRISTINE = empty (no damage condition set)
        const PRISTINE = 0;
        // REALLY_DAMAGED = alias for REALLYDAMAGED (bit 4)
        const REALLY_DAMAGED = Self::REALLYDAMAGED.bits();
        // FIRING_PRIMARY = alias for FIRING_A (bit 36)
        const FIRING_PRIMARY = Self::FIRING_A.bits();
        // FIRING_SECONDARY = alias for FIRING_B (bit 40)
        const FIRING_SECONDARY = Self::FIRING_B.bits();
        // FIRING_TERTIARY = alias for FIRING_C (bit 44)
        const FIRING_TERTIARY = Self::FIRING_C.bits();
        // SELECTED — UI-only flag, not a C++ model condition (bit 118)
        const SELECTED = 1u128 << 118;
        // WEAPON_UPGRADED — game-logic flag, not a C++ model condition (bit 119)
        const WEAPON_UPGRADED = 1u128 << 119;
        // ARMOR_UPGRADED — game-logic flag, not a C++ model condition (bit 120)
        const ARMOR_UPGRADED = 1u128 << 120;
    }
}

impl ModelConditionFlags {
    pub fn clear(&mut self) {
        *self = Self::empty();
    }
}

/// Type alias for singular ModelConditionFlag usage (matches C++ API)
/// This allows code to use ModelConditionFlag::CenterToRight, etc.
pub type ModelConditionFlag = ModelConditionFlags;

// Individual ObjectStatus constants for easier use (matching C++ enum values)
pub const OBJECT_STATUS_NONE: ObjectStatusTypes = ObjectStatusTypes::None;
pub const OBJECT_STATUS_MASKED: ObjectStatusTypes = ObjectStatusTypes::Masked;
pub const OBJECT_STATUS_CAN_STEALTH: ObjectStatusTypes = ObjectStatusTypes::CanStealth;
pub const OBJECT_STATUS_RECONSTRUCTING: ObjectStatusTypes = ObjectStatusTypes::Reconstructing;
pub const OBJECT_STATUS_UNDER_CONSTRUCTION: ObjectStatusTypes =
    ObjectStatusTypes::UnderConstruction;
pub const OBJECT_STATUS_SOLD: ObjectStatusTypes = ObjectStatusTypes::Sold;

// Individual ModelCondition constants for easier use (matching C++ enum values)
pub const MODELCONDITION_PARACHUTING: ModelConditionFlags = ModelConditionFlags::PARACHUTING;
pub const MODELCONDITION_FREEFALL: ModelConditionFlags = ModelConditionFlags::FREEFALL;
pub const MODELCONDITION_PRONE: ModelConditionFlags = ModelConditionFlags::PRONE;
pub const MODELCONDITION_PANICKING: ModelConditionFlags = ModelConditionFlags::PANICKING;
pub const MODELCONDITION_EXPLODED_FLAILING: ModelConditionFlags =
    ModelConditionFlags::EXPLODED_FLAILING;
pub const MODELCONDITION_EXPLODED_BOUNCING: ModelConditionFlags =
    ModelConditionFlags::EXPLODED_BOUNCING;
pub const MODELCONDITION_SPLATTED: ModelConditionFlags = ModelConditionFlags::SPLATTED;
pub const MODELCONDITION_STUNNED_FLAILING: ModelConditionFlags =
    ModelConditionFlags::STUNNED_FLAILING;
pub const MODELCONDITION_STUNNED: ModelConditionFlags = ModelConditionFlags::STUNNED;
pub const MODELCONDITION_CAPTURED: ModelConditionFlags = ModelConditionFlags::CAPTURED;
pub const MODELCONDITION_RUBBLE: ModelConditionFlags = ModelConditionFlags::RUBBLE;
pub const MODELCONDITION_TOPPLED: ModelConditionFlags = ModelConditionFlags::TOPPLED;
pub const MODELCONDITION_FLOODED: ModelConditionFlags = ModelConditionFlags::FLOODED;
pub const MODELCONDITION_CLIMBING: ModelConditionFlags = ModelConditionFlags::CLIMBING;
pub const MODELCONDITION_RAPPELLING: ModelConditionFlags = ModelConditionFlags::RAPPELLING;
pub const MODELCONDITION_ENEMYNEAR: ModelConditionFlags = ModelConditionFlags::ENEMYNEAR;
pub const MODELCONDITION_POST_COLLAPSE: ModelConditionFlags = ModelConditionFlags::POST_COLLAPSE;
pub const MODELCONDITION_BURNED: ModelConditionFlags = ModelConditionFlags::BURNED;
pub const MODELCONDITION_ACTIVELY_CONSTRUCTING: ModelConditionFlags =
    ModelConditionFlags::ActivelyConstructing;
pub const MODELCONDITION_DOOR_1_OPENING: ModelConditionFlags = ModelConditionFlags::Door1Opening;
pub const MODELCONDITION_DOOR_1_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door1WaitingOpen;
pub const MODELCONDITION_DOOR_1_CLOSING: ModelConditionFlags = ModelConditionFlags::Door1Closing;
pub const MODELCONDITION_DOOR_2_OPENING: ModelConditionFlags = ModelConditionFlags::Door2Opening;
pub const MODELCONDITION_DOOR_2_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door2WaitingOpen;
pub const MODELCONDITION_DOOR_2_CLOSING: ModelConditionFlags = ModelConditionFlags::Door2Closing;
pub const MODELCONDITION_DOOR_3_OPENING: ModelConditionFlags = ModelConditionFlags::Door3Opening;
pub const MODELCONDITION_DOOR_3_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door3WaitingOpen;
pub const MODELCONDITION_DOOR_3_CLOSING: ModelConditionFlags = ModelConditionFlags::Door3Closing;
pub const MODELCONDITION_DOOR_4_OPENING: ModelConditionFlags = ModelConditionFlags::Door4Opening;
pub const MODELCONDITION_DOOR_4_WAITING_OPEN: ModelConditionFlags =
    ModelConditionFlags::Door4WaitingOpen;
pub const MODELCONDITION_DOOR_4_CLOSING: ModelConditionFlags = ModelConditionFlags::Door4Closing;
pub const MODELCONDITION_DOOR_1_WAITING_TO_CLOSE: ModelConditionFlags =
    ModelConditionFlags::Door1WaitingToClose;
pub const MODELCONDITION_JETAFTERBURNER: ModelConditionFlags = ModelConditionFlags::JETAFTERBURNER;
pub const MODELCONDITION_JETEXHAUST: ModelConditionFlags = ModelConditionFlags::JETEXHAUST;
pub const MODELCONDITION_SPECIAL_CHEERING: ModelConditionFlags =
    ModelConditionFlags::SPECIAL_CHEERING;
pub const MODELCONDITION_SPECIAL_DAMAGED: ModelConditionFlags =
    ModelConditionFlags::SPECIAL_DAMAGED;
pub const MODELCONDITION_ATTACKING: ModelConditionFlags = ModelConditionFlags::ATTACKING;
pub const MODELCONDITION_DYING: ModelConditionFlags = ModelConditionFlags::DYING;
pub const MODELCONDITION_CARRYING: ModelConditionFlags = ModelConditionFlags::CARRYING;
pub const MODELCONDITION_DEPLOYED: ModelConditionFlags = ModelConditionFlags::DEPLOYED;
pub const MODELCONDITION_MOVING: ModelConditionFlags = ModelConditionFlags::MOVING;
pub const MODELCONDITION_PACKING: ModelConditionFlags = ModelConditionFlags::Packing;
pub const MODELCONDITION_UNPACKING: ModelConditionFlags = ModelConditionFlags::Unpacking;
pub const MODELCONDITION_OVER_WATER: ModelConditionFlags = ModelConditionFlags::OVER_WATER;
pub const MODELCONDITION_SOLD: ModelConditionFlags = ModelConditionFlags::SOLD;
pub const MODELCONDITION_ARMED: ModelConditionFlags = ModelConditionFlags::ARMED;
pub const MODELCONDITION_SECOND_LIFE: ModelConditionFlags = ModelConditionFlags::SECOND_LIFE;
pub const MODELCONDITION_JAMMED: ModelConditionFlags = ModelConditionFlags::JAMMED;
pub const MODELCONDITION_WEAPONSET_VETERAN: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_VETERAN;
pub const MODELCONDITION_WEAPONSET_ELITE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_ELITE;
pub const MODELCONDITION_WEAPONSET_HERO: ModelConditionFlags = ModelConditionFlags::WEAPONSET_HERO;
pub const MODELCONDITION_WEAPONSET_CRATEUPGRADE_ONE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_CRATEUPGRADE_ONE;
pub const MODELCONDITION_WEAPONSET_CRATEUPGRADE_TWO: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_CRATEUPGRADE_TWO;
pub const MODELCONDITION_WEAPONSET_PLAYER_UPGRADE: ModelConditionFlags =
    ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE;
pub const MODELCONDITION_RIDER1: ModelConditionFlags = ModelConditionFlags::RIDER1;
pub const MODELCONDITION_RIDER2: ModelConditionFlags = ModelConditionFlags::RIDER2;
pub const MODELCONDITION_RIDER3: ModelConditionFlags = ModelConditionFlags::RIDER3;
pub const MODELCONDITION_RIDER4: ModelConditionFlags = ModelConditionFlags::RIDER4;
pub const MODELCONDITION_RIDER5: ModelConditionFlags = ModelConditionFlags::RIDER5;
pub const MODELCONDITION_RIDER6: ModelConditionFlags = ModelConditionFlags::RIDER6;
pub const MODELCONDITION_RIDER7: ModelConditionFlags = ModelConditionFlags::RIDER7;
pub const MODELCONDITION_RIDER8: ModelConditionFlags = ModelConditionFlags::RIDER8;
pub const MODELCONDITION_DOCKING: ModelConditionFlags = ModelConditionFlags::DOCKING;
pub const MODELCONDITION_DOCKING_BEGINNING: ModelConditionFlags =
    ModelConditionFlags::DOCKING_BEGINNING;
pub const MODELCONDITION_DOCKING_ACTIVE: ModelConditionFlags = ModelConditionFlags::DOCKING_ACTIVE;
pub const MODELCONDITION_DOCKING_ENDING: ModelConditionFlags = ModelConditionFlags::DOCKING_ENDING;
pub const MODELCONDITION_PREORDER: ModelConditionFlags = ModelConditionFlags::PREORDER;

// Team and player management
/// Team identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TeamId(pub u8);

impl TeamId {
    /// Neutral/observer team
    pub const NEUTRAL: TeamId = TeamId(0);

    /// Team 1 (first player team)
    pub const TEAM_1: TeamId = TeamId(1);

    /// Team 2 (second player team)  
    pub const TEAM_2: TeamId = TeamId(2);

    /// Creates a new team ID, ensuring it's within valid range
    pub fn new(id: u8) -> Option<TeamId> {
        if id <= MAX_PLAYER_COUNT as u8 {
            Some(TeamId(id))
        } else {
            None
        }
    }

    /// Gets the raw team ID value
    pub fn value(self) -> u8 {
        self.0
    }
}

/// Player identifier
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// Neutral/observer player
    pub const NEUTRAL: PlayerId = PlayerId(0);

    /// First playable player (Player 1 in the original SAGE enums)
    pub const FIRST: PlayerId = PlayerId(1);

    /// Creates a new player ID, ensuring it's within valid range
    pub fn new(id: u8) -> Option<PlayerId> {
        if id <= MAX_PLAYER_COUNT as u8 {
            Some(PlayerId(id))
        } else {
            None
        }
    }

    /// Gets the raw player ID value
    pub fn value(self) -> u8 {
        self.0
    }

    /// Returns the wrapped value (compatibility with the C++ `Get()` helper).
    pub fn get(self) -> u8 {
        self.value()
    }

    /// Returns the wrapped value as a `u32` for systems that key by `u32`.
    pub fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl std::fmt::Debug for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlayerId({})", self.0)
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Geometry and positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryExtentModType {
    Type,
    Major,
    Minor,
    Height,
}

pub(crate) fn geometry_type_to_u32(geometry_type: EngineGeometryType) -> u32 {
    match geometry_type {
        EngineGeometryType::Sphere => 0,
        EngineGeometryType::Cylinder => 1,
        EngineGeometryType::Box => 2,
    }
}

pub(crate) fn geometry_type_from_u32(value: u32) -> EngineGeometryType {
    match value {
        0 => EngineGeometryType::Sphere,
        1 => EngineGeometryType::Cylinder,
        2 => EngineGeometryType::Box,
        _ => EngineGeometryType::Box,
    }
}

/// Geometry information (matching C++ GeometryInfo)
#[derive(Debug, Clone)]
pub struct GeometryInfo {
    pub position: Coord3D,
    pub angle: Real,
    pub bounds: AABox,
    pub height_above_terrain: Real,
    pub geometry_type: EngineGeometryType,
    pub is_small: bool,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Coord3D::origin(),
            angle: 0.0,
            bounds: AABox::default(),
            height_above_terrain: 0.0,
            geometry_type: EngineGeometryType::Box,
            is_small: false,
        }
    }
}

impl GeometryInfo {
    pub fn get_geometry_type(&self) -> EngineGeometryType {
        self.geometry_type
    }

    pub fn set_geometry_type(&mut self, geometry_type: EngineGeometryType) {
        self.geometry_type = geometry_type;
    }

    pub fn get_is_small(&self) -> bool {
        self.is_small
    }

    pub fn set_is_small(&mut self, is_small: bool) {
        self.is_small = is_small;
    }

    /// Get the bounding sphere radius (3D, includes height)
    pub fn get_bounding_sphere_radius(&self) -> Real {
        let dx = self.bounds.max.x - self.bounds.min.x;
        let dy = self.bounds.max.y - self.bounds.min.y;
        let dz = self.bounds.max.z - self.bounds.min.z;
        ((dx * dx + dy * dy + dz * dz).sqrt() / 2.0).max(0.0)
    }

    /// Get the bounding circle radius (2D, XY plane only)
    pub fn get_bounding_circle_radius(&self) -> Real {
        let dx = self.bounds.max.x - self.bounds.min.x;
        let dy = self.bounds.max.y - self.bounds.min.y;
        ((dx * dx + dy * dy).sqrt() / 2.0).max(0.0)
    }

    /// Get the major radius (largest XY half-extent).
    pub fn get_major_radius(&self) -> Real {
        let dx = (self.bounds.max.x - self.bounds.min.x).abs();
        let dy = (self.bounds.max.y - self.bounds.min.y).abs();
        (dx.max(dy) * 0.5).max(0.0)
    }

    /// Get the minor radius (smallest XY half-extent).
    pub fn get_minor_radius(&self) -> Real {
        let dx = (self.bounds.max.x - self.bounds.min.x).abs();
        let dy = (self.bounds.max.y - self.bounds.min.y).abs();
        (dx.min(dy) * 0.5).max(0.0)
    }

    /// Get max height above position (matches C++ geometry max height).
    pub fn get_max_height_above_position(&self) -> Real {
        self.bounds.max.z
    }

    /// Get max height below position (matches C++ GeometryInfo::getMaxHeightBelowPosition).
    pub fn get_max_height_below_position(&self) -> Real {
        let below = -self.bounds.min.z;
        if below < 0.0 {
            0.0
        } else {
            below
        }
    }

    /// Get the geometry center position given a base position.
    pub fn get_center_position(&self, pos: &Coord3D) -> Coord3D {
        Coord3D::new(
            pos.x + (self.bounds.min.x + self.bounds.max.x) * 0.5,
            pos.y + (self.bounds.min.y + self.bounds.max.y) * 0.5,
            pos.z + (self.bounds.min.z + self.bounds.max.z) * 0.5,
        )
    }

    /// Calculate min/max pitches from this geometry at `this_pos` to `that` at `that_pos`.
    /// Matches C++ GeometryInfo::calcPitches (Geometry.cpp).
    pub fn calc_pitches(
        &self,
        this_pos: &Coord3D,
        that: &GeometryInfo,
        that_pos: &Coord3D,
    ) -> (Real, Real) {
        let this_center = self.get_center_position(this_pos);
        let dxy =
            ((that_pos.x - this_center.x).powi(2) + (that_pos.y - this_center.y).powi(2)).sqrt();

        let dz_max = (that_pos.z + that.get_max_height_above_position()) - this_center.z;
        let max_pitch = dz_max.atan2(dxy);

        let dz_min = (that_pos.z - that.get_max_height_below_position()) - this_center.z;
        let min_pitch = dz_min.atan2(dxy);

        (min_pitch, max_pitch)
    }

    pub fn tweak_extents(
        &mut self,
        extent_mod_type: GeometryExtentModType,
        extent_mod_amount: Real,
    ) {
        match extent_mod_type {
            GeometryExtentModType::Major => {
                let center_x = (self.bounds.min.x + self.bounds.max.x) * 0.5;
                let center_y = (self.bounds.min.y + self.bounds.max.y) * 0.5;
                let half_x = (self.bounds.max.x - self.bounds.min.x).abs() * 0.5;
                let half_y = (self.bounds.max.y - self.bounds.min.y).abs() * 0.5;
                let radius = self.get_major_radius() + extent_mod_amount;

                if half_x >= half_y {
                    self.bounds.min.x = center_x - radius;
                    self.bounds.max.x = center_x + radius;
                } else {
                    self.bounds.min.y = center_y - radius;
                    self.bounds.max.y = center_y + radius;
                }
            }
            GeometryExtentModType::Minor => {
                let center_x = (self.bounds.min.x + self.bounds.max.x) * 0.5;
                let center_y = (self.bounds.min.y + self.bounds.max.y) * 0.5;
                let half_x = (self.bounds.max.x - self.bounds.min.x).abs() * 0.5;
                let half_y = (self.bounds.max.y - self.bounds.min.y).abs() * 0.5;
                let radius = self.get_minor_radius() + extent_mod_amount;

                if half_x <= half_y {
                    self.bounds.min.x = center_x - radius;
                    self.bounds.max.x = center_x + radius;
                } else {
                    self.bounds.min.y = center_y - radius;
                    self.bounds.max.y = center_y + radius;
                }
            }
            GeometryExtentModType::Height => {
                self.bounds.max.z = self.get_max_height_above_position() + extent_mod_amount;
                if self.bounds.max.z < self.bounds.min.z {
                    self.bounds.min.z = self.bounds.max.z;
                }
            }
            GeometryExtentModType::Type => {
                self.geometry_type = match self.geometry_type {
                    EngineGeometryType::Sphere => EngineGeometryType::Cylinder,
                    EngineGeometryType::Cylinder => EngineGeometryType::Box,
                    EngineGeometryType::Box => EngineGeometryType::Sphere,
                };
            }
        }

        self.is_small = false;
    }

    pub fn get_descriptive_string(&self) -> String {
        format!(
            "{}/{}({} {} {})",
            geometry_type_to_u32(self.geometry_type),
            self.is_small as u32,
            self.get_major_radius(),
            self.get_minor_radius(),
            self.get_max_height_above_position()
        )
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone)]
pub struct AABox {
    pub min: Coord3D,
    pub max: Coord3D,
}

impl Default for AABox {
    fn default() -> Self {
        Self {
            min: Coord3D::origin(),
            max: Coord3D::origin(),
        }
    }
}

// Money and resources
/// Money/resource amount type
pub type Money = i32;

/// Health points type
pub type HealthPoints = f32;

/// Angle in radians
pub type Angle = f32;

/// Distance measurement
pub type Distance = f32;

/// Percentage value (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Percentage(f32);

impl Percentage {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn value(self) -> f32 {
        self.0
    }

    pub fn from_percent(percent: f32) -> Self {
        Self::new(percent / 100.0)
    }

    pub fn to_percent(self) -> f32 {
        self.0 * 100.0
    }
}

// Enumeration types matching C++ definitions

/// Object status types (matching C++ ObjectStatusTypes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ObjectStatusTypes {
    None = 0,
    Destroyed,
    CanAttack,
    UnderConstruction,
    Unselectable,
    NoCollisions,
    NoAttack,
    AirborneTarget,
    Parachuting,
    Repulsor,
    Hijacked,
    Aflame,
    Burned,
    Wet,
    IsFiringWeapon,
    Braking,
    Stealthed,
    Detected,
    CanStealth,
    Sold,
    UndergoingRepair,
    Reconstructing,
    Masked,
    IsAttacking,
    IsUsingAbility,
    IsAimingWeapon,
    NoAttackFromAi,
    IgnoringStealth,
    IsCarBomb,
    DeckHeightOffset,
    Rider1,
    Rider2,
    Rider3,
    Rider4,
    Rider5,
    Rider6,
    Rider7,
    Rider8,
    FaerieFire,
    MissileKillingSelf,
    ReassignParking,
    BoobyTrapped,
    Immobile,
    Disguised,
    Deployed,
}

impl ObjectStatusTypes {
    /// Convert from a raw integer value, defaulting to `ObjectStatusTypes::None`.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => ObjectStatusTypes::None,
            1 => ObjectStatusTypes::Destroyed,
            2 => ObjectStatusTypes::CanAttack,
            3 => ObjectStatusTypes::UnderConstruction,
            4 => ObjectStatusTypes::Unselectable,
            5 => ObjectStatusTypes::NoCollisions,
            6 => ObjectStatusTypes::NoAttack,
            7 => ObjectStatusTypes::AirborneTarget,
            8 => ObjectStatusTypes::Parachuting,
            9 => ObjectStatusTypes::Repulsor,
            10 => ObjectStatusTypes::Hijacked,
            11 => ObjectStatusTypes::Aflame,
            12 => ObjectStatusTypes::Burned,
            13 => ObjectStatusTypes::Wet,
            14 => ObjectStatusTypes::IsFiringWeapon,
            15 => ObjectStatusTypes::Braking,
            16 => ObjectStatusTypes::Stealthed,
            17 => ObjectStatusTypes::Detected,
            18 => ObjectStatusTypes::CanStealth,
            19 => ObjectStatusTypes::Sold,
            20 => ObjectStatusTypes::UndergoingRepair,
            21 => ObjectStatusTypes::Reconstructing,
            22 => ObjectStatusTypes::Masked,
            23 => ObjectStatusTypes::IsAttacking,
            24 => ObjectStatusTypes::IsUsingAbility,
            25 => ObjectStatusTypes::IsAimingWeapon,
            26 => ObjectStatusTypes::NoAttackFromAi,
            27 => ObjectStatusTypes::IgnoringStealth,
            28 => ObjectStatusTypes::IsCarBomb,
            29 => ObjectStatusTypes::DeckHeightOffset,
            30 => ObjectStatusTypes::Rider1,
            31 => ObjectStatusTypes::Rider2,
            32 => ObjectStatusTypes::Rider3,
            33 => ObjectStatusTypes::Rider4,
            34 => ObjectStatusTypes::Rider5,
            35 => ObjectStatusTypes::Rider6,
            36 => ObjectStatusTypes::Rider7,
            37 => ObjectStatusTypes::Rider8,
            38 => ObjectStatusTypes::FaerieFire,
            39 => ObjectStatusTypes::MissileKillingSelf,
            40 => ObjectStatusTypes::ReassignParking,
            41 => ObjectStatusTypes::BoobyTrapped,
            42 => ObjectStatusTypes::Immobile,
            43 => ObjectStatusTypes::Disguised,
            44 => ObjectStatusTypes::Deployed,
            _ => ObjectStatusTypes::None,
        }
    }

    // Legacy C++-style aliases used by in-progress ported call sites.
    pub const OBJECT_STATUS_IS_ATTACKING: ObjectStatusTypes = ObjectStatusTypes::IsAttacking;
}

/// Disabled types (matching C++ DisabledType order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledType {
    DisabledDefault,
    DisabledHacked,
    DisabledEmp,
    Held,
    Paralyzed,
    DisabledUnmanned,
    DisabledUnderpowered,
    DisabledFreefall,
    DisabledAwestruck,
    DisabledBrainwashed,
    DisabledSubdued,
    DisabledScriptDisabled,
    DisabledScriptUnderpowered,
    DisabledAny,
    Unmanned, // Alias for DisabledUnmanned
}

/// Weapon set types (matching C++ WeaponSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSetType {
    Primary,
    Secondary,
    Tertiary,
    Passenger,
}

/// Weapon slot types (matching C++ WeaponSlotType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

impl WeaponSlotType {
    /// Convert from u32 value (matches C++ casting)
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(WeaponSlotType::Primary),
            1 => Some(WeaponSlotType::Secondary),
            2 => Some(WeaponSlotType::Tertiary),
            _ => None,
        }
    }
}

/// Weapon lock types (matching C++ WeaponLockType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponLockType {
    None,
    Acquiring,
    Locked,
    LockedTemporarily,
}

bitflags! {
    /// Script-driven status overrides (`ObjectScriptStatusBit` in C++).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectScriptStatusBits: u8 {
        const DISABLED    = 0x01;
        const UNPOWERED   = 0x02;
        const UNSELLABLE  = 0x04;
        const UNSTEALTHED = 0x08;
        const TARGETABLE  = 0x10;
    }
}

/// Object shroud state (`ObjectShroudStatus` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjectShroudStatus {
    Invalid = 0,
    Clear = 1,
    PartialClear = 2,
    Fogged = 3,
    Shrouded = 4,
    InvalidButPreviousValid = 5,
}

/// Radar priority levels (`RadarPriorityType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RadarPriorityType {
    Invalid = 0,
    NotOnRadar = 1,
    Structure = 2,
    Unit = 3,
    LocalUnitOnly = 4,
}

// Re-export canonical CommandSourceType from Common (matches C++ with #[repr(u32)])
pub use game_engine::common::game_common::CommandSourceType;

/// Locomotor set selection (`LocomotorSetType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum LocomotorSetType {
    Invalid = -1,
    Normal = 0,
    NormalUpgraded = 1,
    Freefall = 2,
    Wander = 3,
    Panic = 4,
    Taxiing = 5,
    Supersonic = 6,
    Sluggish = 7,
}

// Re-export AbleToAttackType helpers from Common (C++ bitmask semantics)
pub use game_engine::common::game_common::{
    is_continued_attack, is_forced_attack, AbleToAttackType,
};

/// Turret identifiers (`WhichTurretType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WhichTurretType {
    Invalid = -1,
    Main = 0,
    Alt = 1,
    MaxTurrets = 2,
}

/// Special power identifiers (`SpecialPowerType` in C++).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SpecialPowerType {
    SpecialInvalid = 0,
    SpecialDaisyCutter,
    SpecialParadropAmerica,
    SpecialCarpetBomb,
    SpecialClusterMines,
    SpecialEmpPulse,
    SpecialNapalmStrike,
    SpecialCashHack,
    SpecialNeutronMissile,
    SpecialSpySatellite,
    SpecialDefector,
    SpecialTerrorCell,
    SpecialAmbush,
    SpecialBlackMarketNuke,
    SpecialAnthraxBomb,
    SpecialScudStorm,
    SpecialDemoralizeObsolete,
    SpecialCrateDrop,
    SpecialA10ThunderboltStrike,
    SpecialDetonateDirtyNuke,
    SpecialArtilleryBarrage,
    SpecialMissileDefenderLaserGuidedMissiles,
    SpecialRemoteCharges,
    SpecialTimedCharges,
    SpecialHelixNapalmBomb,
    SpecialHackerDisableBuilding,
    SpecialTankHunterTntAttack,
    SpecialBlackLotusCaptureBuilding,
    SpecialBlackLotusDisableVehicleHack,
    SpecialBlackLotusStealCashHack,
    SpecialInfantryCaptureBuilding,
    SpecialRadarVanScan,
    SpecialSpyDrone,
    SpecialDisguiseAsVehicle,
    SpecialBoobyTrap,
    SpecialRepairVehicles,
    SpecialParticleUplinkCannon,
    SpecialCashBounty,
    SpecialChangeBattlePlans,
    SpecialCiaIntelligence,
    SpecialCleanupArea,
    SpecialLaunchBaikonurRocket,
    SpecialSpectreGunship,
    SpecialGpsScrambler,
    SpecialFrenzy,
    SpecialSneakAttack,
    SpecialChinaCarpetBomb,
    EarlySpecialChinaCarpetBomb,
    SpecialLeafletDrop,
    EarlySpecialLeafletDrop,
    EarlySpecialFrenzy,
    SpecialCommunicationsDownload,
    EarlySpecialRepairVehicles,
    SpecialTankParadrop,
    SupwSpecialParticleUplinkCannon,
    AirfSpecialDaisyCutter,
    NukeSpecialClusterMines,
    NukeSpecialNeutronMissile,
    AirfSpecialA10ThunderboltStrike,
    AirfSpecialSpectreGunship,
    InfaSpecialParadropAmerica,
    SlthSpecialGpsScrambler,
    AirfSpecialCarpetBomb,
    SuprSpecialCruiseMissile,
    LazrSpecialParticleUplinkCannon,
    SupwSpecialNeutronMissile,
    SpecialBattleshipBombardment,
    SpecialPowerCount,
}

impl SpecialPowerType {
    /// Convert from a numeric value (matches C++ casting with bounds check).
    pub fn from_u32(value: u32) -> Option<Self> {
        if value <= SpecialPowerType::SpecialPowerCount as u32 {
            // SAFETY: SpecialPowerType is #[repr(u32)] and we bounds-check against the max value.
            Some(unsafe { std::mem::transmute(value) })
        } else {
            None
        }
    }
}

/// Weapon choice criteria (matching C++ WeaponChoiceCriteria)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponChoiceCriteria {
    Prefer,
    RequireToFire,
    PreferMostDamage,
}

/// Weapon bonus condition type (matching C++ WeaponBonusConditionType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponBonusConditionType {
    Invalid,
    Garrisoned,
    Horde,
    ContinuousFireMean,
    ContinuousFireFast,
    Nationalism,
    PlayerUpgrade,
    DroneSpotting,
    Demoralized,
    DemoralizedObsolete,
    Enthusiastic,
    Veteran,
    Elite,
    Hero,
    BattlePlanBombardment,
    BattlePlanHoldTheLine,
    BattlePlanSearchAndDestroy,
    Subliminal,
    SoloHumanEasy,
    SoloHumanNormal,
    SoloHumanHard,
    SoloAiEasy,
    SoloAiNormal,
    SoloAiHard,
    TargetFaerieFire,
    Fanaticism,
    FrenzyOne,
    FrenzyTwo,
    FrenzyThree,
    DroneSpotForStrike,
}

/// Armor set type (matching C++ ArmorSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSetType {
    Default,
    Veteran,
    Elite,
    Hero,
    Player,
    CrateUpgradeOne,
    CrateUpgradeTwo,
}

/// Weapon status (matching C++ WeaponStatus)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    Ready,
    BetweenShots,
    Reloading,
    PreAttack,
}

/// Pathfind layer enum (matching C++ PathfindLayerEnum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    Bridge1 = 3,
    Bridge2 = 4,
    Bridge3 = 5,
    Bridge4 = 6,
    Wall = 7,
    Tunnel = 8,
    Water = 9,
    Air = 10,
    Last = 11, // Used for array bounds
}

impl PathfindLayerEnum {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => PathfindLayerEnum::Invalid,
            1 => PathfindLayerEnum::Ground,
            2 => PathfindLayerEnum::Top,
            3 => PathfindLayerEnum::Bridge1,
            4 => PathfindLayerEnum::Bridge2,
            5 => PathfindLayerEnum::Bridge3,
            6 => PathfindLayerEnum::Bridge4,
            7 => PathfindLayerEnum::Wall,
            8 => PathfindLayerEnum::Tunnel,
            9 => PathfindLayerEnum::Water,
            10 => PathfindLayerEnum::Air,
            11 => PathfindLayerEnum::Last,
            _ => PathfindLayerEnum::Invalid,
        }
    }
}

/// Formation ID (matching C++ `FormationID`).
///
/// In the original engine this is an opaque, per-group identifier assigned by the AI system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormationID(u32);

impl FormationID {
    pub const NONE: FormationID = FormationID(0);

    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl Default for FormationID {
    fn default() -> Self {
        FormationID::NONE
    }
}

/// Veterancy level (matching C++ VeterancyLevel)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl VeterancyLevel {
    pub fn saturating_add_levels(self, delta: i32) -> Self {
        let raw = self as i32;
        let min = VeterancyLevel::Regular as i32;
        let max = VeterancyLevel::Heroic as i32;
        let clamped = (raw + delta).clamp(min, max);
        match clamped {
            0 => VeterancyLevel::Regular,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            _ => VeterancyLevel::Heroic,
        }
    }
}

impl fmt::Display for VeterancyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            VeterancyLevel::Regular => "Regular",
            VeterancyLevel::Veteran => "Veteran",
            VeterancyLevel::Elite => "Elite",
            VeterancyLevel::Heroic => "Heroic",
        };
        write!(f, "{}", label)
    }
}

// Re-export canonical Relationship from Common (Enemies=0, Neutral=1, Allies=2 matching C++)
pub use game_engine::common::game_common::Relationship;

// Re-export canonical DamageType/DeathType from damage_system (38/21 C++-correct variants)
pub use crate::weapon::damage_system::{DamageType, DeathType};

/// Kind of classifications for objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindOf {
    Selectable,
    Unit,
    Building,
    Vehicle,
    Infantry,
    Aircraft,
    Drone,
    CliffJumper,
    Structure,
    Weapon,
    Projectile,
    CanSeeThrough,
    AlwaysSelectable,
    Crate,
    ResourceNode,
    SupplySourceOnPreview,
    SupplySource,
    TechBuilding,
    Powered,
    ProducedAtHelipad,
    Bridge,
    Barrier,
    Civilian,
    Destructible,
    CanCrossBridges,
    Amphibious,
    AmphibiousTransport,
    Transport,
    CanCapture,
    Saboteur,
    Hacker,
    Hero,
    KeyStructure,
    CommandCenter,
    Prison,
    CollectsPrisonBounty,
    PowTruck,
    PowerPlant,
    Refinery,
    Factory,
    Defense,
    Shrubbery,
    Dozer,
    Harvester,
    Hulk,
    Salvager,
    WeaponSalvager,
    ArmorSalvager,
    AircraftCarrier,
    FSBarracks,
    FSWarfactory,
    FSAirfield,
    FSInternetCenter,
    FSPower,
    FSSupplyDropzone,
    FSSupplyCenter,
    FSSuperweapon,
    FSStrategyCenter,
    FSFake,
    CountsForVictory,
    Mine,
    CleanupHazard,
    HealPad,
    WaveGuide,
    BridgeTower,
    Immobile,
    BoobyTrap,
    Disguiser,
    PortableStructure,
    CanRappel,
    CanBeRepulsed,
    EmpHardened,
    SpawnsAreTheWeapons,
    IgnoreDockingBones,
    CanSurrender,
    RepairPad,
    RejectUnmanned,
    IgnoredInGui,
    MobNexus,
    Capturable,
    ImmuneToCapture,
    CashGenerator,
    RebuildHole,
    FSTechnology,
    NoGarrison,
    Boat,
    GarrisonableUntilDestroyed,
    Obstacle,                   // KINDOF_OBSTACLE (bit 0)
    CanAttack,                  // KINDOF_CAN_ATTACK
    StickToTerrainSlope,        // KINDOF_STICK_TO_TERRAIN_SLOPE
    CanCastReflections,         // KINDOF_CAN_CAST_REFLECTIONS
    HugeVehicle,                // KINDOF_HUGE_VEHICLE
    LineBuild,                  // KINDOF_LINEBUILD
    Preload,                    // KINDOF_PRELOAD
    NoCollide,                  // KINDOF_NO_COLLIDE
    StealthGarrison,            // KINDOF_STEALTH_GARRISON
    DrawableOnly,               // KINDOF_DRAWABLE_ONLY
    Score,                      // KINDOF_SCORE
    ScoreCreate,                // KINDOF_SCORE_CREATE
    ScoreDestroy,               // KINDOF_SCORE_DESTROY
    NoHealIcon,                 // KINDOF_NO_HEAL_ICON
    Parachutable,               // KINDOF_PARACHUTABLE
    SmallMissile,               // KINDOF_SMALL_MISSILE
    AlwaysVisible,              // KINDOF_ALWAYS_VISIBLE
    Unattackable,               // KINDOF_UNATTACKABLE
    AttackNeedsLineOfSight,     // KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT
    WalkOnTopOfWall,            // KINDOF_WALK_ON_TOP_OF_WALL
    DefensiveWall,              // KINDOF_DEFENSIVE_WALL
    AircraftPathAround,         // KINDOF_AIRCRAFT_PATH_AROUND
    LowOverlappable,            // KINDOF_LOW_OVERLAPPABLE
    ForceAttackable,            // KINDOF_FORCEATTACKABLE
    AutoRallypoint,             // KINDOF_AUTO_RALLYPOINT
    MoneyHacker,                // KINDOF_MONEY_HACKER
    BallisticMissile,           // KINDOF_BALLISTIC_MISSILE
    ClickThrough,               // KINDOF_CLICK_THROUGH
    ShowPortraitWhenControlled, // KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED
    CannotBuildNearSupplies,    // KINDOF_CANNOT_BUILD_NEAR_SUPPLIES
    RevealToAll,                // KINDOF_REVEAL_TO_ALL
    IgnoresSelectAll,           // KINDOF_IGNORES_SELECT_ALL
    DontAutoCrushInfantry,      // KINDOF_DONT_AUTO_CRUSH_INFANTRY
    FsBlackMarket,              // KINDOF_FS_BLACK_MARKET
    FsAdvancedTech,             // KINDOF_FS_ADVANCED_TECH
    RevealsEnemyPaths,          // KINDOF_REVEALS_ENEMY_PATHS
    NoSelect,                   // KINDOF_NO_SELECT
    CannotRetaliate,            // KINDOF_CANNOT_RETALIATE
    TechBaseDefense,            // KINDOF_TECH_BASE_DEFENSE
    Demotrap,                   // KINDOF_DEMOTRAP
    ConservativeBuilding,       // KINDOF_CONSERVATIVE_BUILDING
    BlastCrater,                // KINDOF_BLAST_CRATER
    Prop,                       // KINDOF_PROP
    OptimizedTree,              // KINDOF_OPTIMIZED_TREE
    LandmarkBridge,             // KINDOF_LANDMARK_BRIDGE
    WaveEffect,                 // KINDOF_WAVE_EFFECT
    ClearedByBuild,             // KINDOF_CLEARED_BY_BUILD
}

impl KindOf {
    /// Legacy script alias used by original C++ script conditions.
    pub const Inert: KindOf = KindOf::Immobile;
    /// Legacy script alias used by original C++ script actions.
    pub const CanRepair: KindOf = KindOf::RepairPad;
}

/// Resolve a KindOf flag name (as written in INI files) to its enum variant.
///
/// C++ Reference: KindOfMaskType::parseFromINI reads pipe-separated names like
/// `"SELECTABLE STRUCTURE CAN_ATTACK"` and OR's the corresponding bits.
///
/// The names here match the C++ INI token names exactly (uppercase, underscores).
/// We also accept mixed-case for robustness.
pub fn kindof_from_name(name: &str) -> Option<KindOf> {
    // Normalise: strip leading/trailing whitespace, uppercase for comparison
    let upper = name.trim().to_ascii_uppercase();
    match upper.as_str() {
        "SELECTABLE" => Some(KindOf::Selectable),
        "INFANTRY" => Some(KindOf::Infantry),
        "VEHICLE" => Some(KindOf::Vehicle),
        "STRUCTURE" => Some(KindOf::Structure),
        "AIRCRAFT" => Some(KindOf::Aircraft),
        "UNIT" => Some(KindOf::Unit),
        "DRONE" => Some(KindOf::Drone),
        "CLIFF_JUMPER" => Some(KindOf::CliffJumper),
        "WEAPON" => Some(KindOf::Weapon),
        "PROJECTILE" => Some(KindOf::Projectile),
        "CAN_SEE_THROUGH" => Some(KindOf::CanSeeThrough),
        "ALWAYS_SELECTABLE" => Some(KindOf::AlwaysSelectable),
        "CRATE" => Some(KindOf::Crate),
        "RESOURCE_NODE" => Some(KindOf::ResourceNode),
        "SUPPLY_SOURCE_ON_PREVIEW" => Some(KindOf::SupplySourceOnPreview),
        "SUPPLY_SOURCE" => Some(KindOf::SupplySource),
        "TECH_BUILDING" => Some(KindOf::TechBuilding),
        "POWERED" => Some(KindOf::Powered),
        "PRODUCED_AT_HELIPAD" => Some(KindOf::ProducedAtHelipad),
        "BRIDGE" => Some(KindOf::Bridge),
        "BARRIER" => Some(KindOf::Barrier),
        "CIVILIAN" => Some(KindOf::Civilian),
        "DESTRUCTIBLE" => Some(KindOf::Destructible),
        "CAN_CROSS_BRIDGES" => Some(KindOf::CanCrossBridges),
        "AMPHIBIOUS" => Some(KindOf::Amphibious),
        "AMPHIBIOUS_TRANSPORT" => Some(KindOf::AmphibiousTransport),
        "TRANSPORT" => Some(KindOf::Transport),
        "CAN_CAPTURE" => Some(KindOf::CanCapture),
        "SABOTEUR" => Some(KindOf::Saboteur),
        "HACKER" => Some(KindOf::Hacker),
        "HERO" => Some(KindOf::Hero),
        "KEY_STRUCTURE" => Some(KindOf::KeyStructure),
        "COMMAND_CENTER" => Some(KindOf::CommandCenter),
        "PRISON" => Some(KindOf::Prison),
        "COLLECTS_PRISON_BOUNTY" => Some(KindOf::CollectsPrisonBounty),
        "POW_TRUCK" => Some(KindOf::PowTruck),
        "POWER_PLANT" => Some(KindOf::PowerPlant),
        "REFINERY" => Some(KindOf::Refinery),
        "FACTORY" => Some(KindOf::Factory),
        "DEFENSE" => Some(KindOf::Defense),
        "SHRUBBERY" => Some(KindOf::Shrubbery),
        "DOZER" => Some(KindOf::Dozer),
        "HARVESTER" => Some(KindOf::Harvester),
        "HULK" => Some(KindOf::Hulk),
        "SALVAGER" => Some(KindOf::Salvager),
        "WEAPON_SALVAGER" => Some(KindOf::WeaponSalvager),
        "ARMOR_SALVAGER" => Some(KindOf::ArmorSalvager),
        "AIRCRAFT_CARRIER" => Some(KindOf::AircraftCarrier),
        "FS_BARRACKS" => Some(KindOf::FSBarracks),
        "FS_WARFACTORY" => Some(KindOf::FSWarfactory),
        "FS_AIRFIELD" => Some(KindOf::FSAirfield),
        "FS_INTERNET_CENTER" => Some(KindOf::FSInternetCenter),
        "FS_POWER" => Some(KindOf::FSPower),
        "FS_SUPPLY_DROPZONE" => Some(KindOf::FSSupplyDropzone),
        "FS_SUPPLY_CENTER" => Some(KindOf::FSSupplyCenter),
        "FS_SUPERWEAPON" => Some(KindOf::FSSuperweapon),
        "FS_STRATEGY_CENTER" => Some(KindOf::FSStrategyCenter),
        "FS_FAKE" => Some(KindOf::FSFake),
        "FS_BLACK_MARKET" => Some(KindOf::FsBlackMarket),
        "FS_ADVANCED_TECH" => Some(KindOf::FsAdvancedTech),
        "FS_TECHNOLOGY" => Some(KindOf::FSTechnology),
        "COUNTS_FOR_VICTORY" => Some(KindOf::CountsForVictory),
        "MINE" => Some(KindOf::Mine),
        "CLEANUP_HAZARD" => Some(KindOf::CleanupHazard),
        "HEAL_PAD" => Some(KindOf::HealPad),
        "WAVE_GUIDE" => Some(KindOf::WaveGuide),
        "BRIDGE_TOWER" => Some(KindOf::BridgeTower),
        "IMMOBILE" | "INERT" => Some(KindOf::Immobile),
        "BOOBY_TRAP" => Some(KindOf::BoobyTrap),
        "DISGUISER" => Some(KindOf::Disguiser),
        "PORTABLE_STRUCTURE" => Some(KindOf::PortableStructure),
        "CAN_RAPPEL" => Some(KindOf::CanRappel),
        "CAN_BE_REPULSED" => Some(KindOf::CanBeRepulsed),
        "EMP_HARDENED" => Some(KindOf::EmpHardened),
        "SPAWNS_ARE_THE_WEAPONS" => Some(KindOf::SpawnsAreTheWeapons),
        "IGNORE_DOCKING_BONES" => Some(KindOf::IgnoreDockingBones),
        "CAN_SURRENDER" => Some(KindOf::CanSurrender),
        "REPAIR_PAD" | "CAN_REPAIR" => Some(KindOf::RepairPad),
        "REJECT_UNMANNED" => Some(KindOf::RejectUnmanned),
        "IGNORED_IN_GUI" => Some(KindOf::IgnoredInGui),
        "MOB_NEXUS" => Some(KindOf::MobNexus),
        "CAPTURABLE" => Some(KindOf::Capturable),
        "IMMUNE_TO_CAPTURE" => Some(KindOf::ImmuneToCapture),
        "CASH_GENERATOR" => Some(KindOf::CashGenerator),
        "REBUILD_HOLE" => Some(KindOf::RebuildHole),
        "NO_GARRISON" => Some(KindOf::NoGarrison),
        "BOAT" => Some(KindOf::Boat),
        "GARRISONABLE_UNTIL_DESTROYED" => Some(KindOf::GarrisonableUntilDestroyed),
        "OBSTACLE" => Some(KindOf::Obstacle),
        "CAN_ATTACK" => Some(KindOf::CanAttack),
        "STICK_TO_TERRAIN_SLOPE" => Some(KindOf::StickToTerrainSlope),
        "CAN_CAST_REFLECTIONS" => Some(KindOf::CanCastReflections),
        "HUGE_VEHICLE" => Some(KindOf::HugeVehicle),
        "LINEBUILD" | "LINE_BUILD" => Some(KindOf::LineBuild),
        "PRELOAD" => Some(KindOf::Preload),
        "NO_COLLIDE" => Some(KindOf::NoCollide),
        "STEALTH_GARRISON" => Some(KindOf::StealthGarrison),
        "DRAWABLE_ONLY" => Some(KindOf::DrawableOnly),
        "SCORE" => Some(KindOf::Score),
        "SCORE_CREATE" => Some(KindOf::ScoreCreate),
        "SCORE_DESTROY" => Some(KindOf::ScoreDestroy),
        "NO_HEAL_ICON" => Some(KindOf::NoHealIcon),
        "PARACHUTABLE" => Some(KindOf::Parachutable),
        "SMALL_MISSILE" => Some(KindOf::SmallMissile),
        "ALWAYS_VISIBLE" => Some(KindOf::AlwaysVisible),
        "UNATTACKABLE" => Some(KindOf::Unattackable),
        "ATTACK_NEEDS_LINE_OF_SIGHT" => Some(KindOf::AttackNeedsLineOfSight),
        "WALK_ON_TOP_OF_WALL" => Some(KindOf::WalkOnTopOfWall),
        "DEFENSIVE_WALL" => Some(KindOf::DefensiveWall),
        "AIRCRAFT_PATH_AROUND" => Some(KindOf::AircraftPathAround),
        "LOW_OVERLAPPABLE" => Some(KindOf::LowOverlappable),
        "FORCEATTACKABLE" | "FORCE_ATTACKABLE" => Some(KindOf::ForceAttackable),
        "AUTO_RALLYPOINT" | "AUTO_RALLY_POINT" => Some(KindOf::AutoRallypoint),
        "MONEY_HACKER" => Some(KindOf::MoneyHacker),
        "BALLISTIC_MISSILE" => Some(KindOf::BallisticMissile),
        "CLICK_THROUGH" => Some(KindOf::ClickThrough),
        "SHOW_PORTRAIT_WHEN_CONTROLLED" => Some(KindOf::ShowPortraitWhenControlled),
        "CANNOT_BUILD_NEAR_SUPPLIES" => Some(KindOf::CannotBuildNearSupplies),
        "REVEAL_TO_ALL" => Some(KindOf::RevealToAll),
        "IGNORES_SELECT_ALL" => Some(KindOf::IgnoresSelectAll),
        "DONT_AUTO_CRUSH_INFANTRY" => Some(KindOf::DontAutoCrushInfantry),
        "REVEALS_ENEMY_PATHS" => Some(KindOf::RevealsEnemyPaths),
        "NO_SELECT" => Some(KindOf::NoSelect),
        "CANNOT_RETALIATE" => Some(KindOf::CannotRetaliate),
        "TECH_BASE_DEFENSE" => Some(KindOf::TechBaseDefense),
        "DEMOTRAP" | "DEMO_TRAP" => Some(KindOf::Demotrap),
        "CONSERVATIVE_BUILDING" => Some(KindOf::ConservativeBuilding),
        "BLAST_CRATER" => Some(KindOf::BlastCrater),
        "PROP" => Some(KindOf::Prop),
        "OPTIMIZED_TREE" => Some(KindOf::OptimizedTree),
        "LANDMARK_BRIDGE" => Some(KindOf::LandmarkBridge),
        "WAVE_EFFECT" => Some(KindOf::WaveEffect),
        "CLEARED_BY_BUILD" => Some(KindOf::ClearedByBuild),
        _ => None,
    }
}

/// All `KindOf` variants in declaration order.
///
/// This is used by legacy systems that still operate on bitmask representations
/// (`KindOfMaskType`) but only have `is_kind_of(KindOf)` queries available.
pub const ALL_KIND_OF: &[KindOf] = &[
    KindOf::Selectable,
    KindOf::Unit,
    KindOf::Building,
    KindOf::Vehicle,
    KindOf::Infantry,
    KindOf::Aircraft,
    KindOf::Drone,
    KindOf::CliffJumper,
    KindOf::Structure,
    KindOf::Weapon,
    KindOf::Projectile,
    KindOf::CanSeeThrough,
    KindOf::AlwaysSelectable,
    KindOf::Crate,
    KindOf::ResourceNode,
    KindOf::SupplySourceOnPreview,
    KindOf::SupplySource,
    KindOf::TechBuilding,
    KindOf::Powered,
    KindOf::ProducedAtHelipad,
    KindOf::Bridge,
    KindOf::Barrier,
    KindOf::Civilian,
    KindOf::Destructible,
    KindOf::CanCrossBridges,
    KindOf::Amphibious,
    KindOf::AmphibiousTransport,
    KindOf::Transport,
    KindOf::CanCapture,
    KindOf::Saboteur,
    KindOf::Hacker,
    KindOf::Hero,
    KindOf::KeyStructure,
    KindOf::CommandCenter,
    KindOf::Prison,
    KindOf::CollectsPrisonBounty,
    KindOf::PowTruck,
    KindOf::PowerPlant,
    KindOf::Refinery,
    KindOf::Factory,
    KindOf::Defense,
    KindOf::Shrubbery,
    KindOf::Dozer,
    KindOf::Harvester,
    KindOf::Hulk,
    KindOf::Salvager,
    KindOf::WeaponSalvager,
    KindOf::ArmorSalvager,
    KindOf::AircraftCarrier,
    KindOf::FSBarracks,
    KindOf::FSWarfactory,
    KindOf::FSAirfield,
    KindOf::FSInternetCenter,
    KindOf::FSPower,
    KindOf::FSSupplyDropzone,
    KindOf::FSSupplyCenter,
    KindOf::FSSuperweapon,
    KindOf::FSStrategyCenter,
    KindOf::FSFake,
    KindOf::CountsForVictory,
    KindOf::Mine,
    KindOf::CleanupHazard,
    KindOf::HealPad,
    KindOf::WaveGuide,
    KindOf::BridgeTower,
    KindOf::Immobile,
    KindOf::BoobyTrap,
    KindOf::Disguiser,
    KindOf::PortableStructure,
    KindOf::CanRappel,
    KindOf::CanBeRepulsed,
    KindOf::EmpHardened,
    KindOf::SpawnsAreTheWeapons,
    KindOf::IgnoreDockingBones,
    KindOf::CanSurrender,
    KindOf::RepairPad,
    KindOf::RejectUnmanned,
    KindOf::IgnoredInGui,
    KindOf::MobNexus,
    KindOf::Capturable,
    KindOf::ImmuneToCapture,
    KindOf::CashGenerator,
    KindOf::RebuildHole,
    KindOf::FSTechnology,
    KindOf::NoGarrison,
    KindOf::Boat,
    KindOf::GarrisonableUntilDestroyed,
    KindOf::Obstacle,
    KindOf::CanAttack,
    KindOf::StickToTerrainSlope,
    KindOf::CanCastReflections,
    KindOf::HugeVehicle,
    KindOf::LineBuild,
    KindOf::Preload,
    KindOf::NoCollide,
    KindOf::StealthGarrison,
    KindOf::DrawableOnly,
    KindOf::Score,
    KindOf::ScoreCreate,
    KindOf::ScoreDestroy,
    KindOf::NoHealIcon,
    KindOf::Parachutable,
    KindOf::SmallMissile,
    KindOf::AlwaysVisible,
    KindOf::Unattackable,
    KindOf::AttackNeedsLineOfSight,
    KindOf::WalkOnTopOfWall,
    KindOf::DefensiveWall,
    KindOf::AircraftPathAround,
    KindOf::LowOverlappable,
    KindOf::ForceAttackable,
    KindOf::AutoRallypoint,
    KindOf::MoneyHacker,
    KindOf::BallisticMissile,
    KindOf::ClickThrough,
    KindOf::ShowPortraitWhenControlled,
    KindOf::CannotBuildNearSupplies,
    KindOf::RevealToAll,
    KindOf::IgnoresSelectAll,
    KindOf::DontAutoCrushInfantry,
    KindOf::FsBlackMarket,
    KindOf::FsAdvancedTech,
    KindOf::RevealsEnemyPaths,
    KindOf::NoSelect,
    KindOf::CannotRetaliate,
    KindOf::TechBaseDefense,
    KindOf::Demotrap,
    KindOf::ConservativeBuilding,
    KindOf::BlastCrater,
    KindOf::Prop,
    KindOf::OptimizedTree,
    KindOf::LandmarkBridge,
    KindOf::WaveEffect,
    KindOf::ClearedByBuild,
];

/// Team member list type (matching C++ MAKE_DLINK)
pub type TeamMemberList = Vec<ObjectID>;

// Map and terrain related types

/// Waypoint ID type (matching C++ WaypointID)
pub type WaypointID = u32;

/// Invalid waypoint ID constant  
pub const INVALID_WAYPOINT_ID: WaypointID = 0x7FFFFFFF;

/// Body damage type (matching C++ BodyDamageType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyDamageType {
    Pristine,
    Damaged,
    ReallyDamaged,
    Rubble,
}

/// Bridge tower type (matching C++ BridgeTowerType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeTowerType {
    From = 0,
    To = 1,
}

/// Maximum number of bridge towers
pub const BRIDGE_MAX_TOWERS: usize = 2;

/// 2D region (matching C++ Region2D)
#[derive(Debug, Clone, Copy)]
pub struct Region2D {
    pub lo: Coord2D,
    pub hi: Coord2D,
}

impl Default for Region2D {
    fn default() -> Self {
        Self {
            lo: Coord2D::ZERO,
            hi: Coord2D::ZERO,
        }
    }
}

impl Region2D {
    pub fn new(lo: Coord2D, hi: Coord2D) -> Self {
        Self { lo, hi }
    }
}

/// Integer 2D region (matching C++ IRegion2D)  
#[derive(Debug, Clone, Copy)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl Default for IRegion2D {
    fn default() -> Self {
        Self {
            lo: ICoord2D::ZERO,
            hi: ICoord2D::ZERO,
        }
    }
}

impl IRegion2D {
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }
}

/// 3D region (matching C++ Region3D)
#[derive(Debug, Clone, Copy)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Default for Region3D {
    fn default() -> Self {
        Self {
            lo: Coord3D::origin(),
            hi: Coord3D::origin(),
        }
    }
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }
}

/// Map dimensions and scaling constants (matching C++ definitions)
pub const MAP_XY_FACTOR: f32 = 10.0; // How wide and tall each height map square is in world space
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0; // Divide all map heights by 8

/// Pathfind cell size constants

/// Locomotor surface type mask (matching C++ LocomotorSurfaceTypeMask)
pub type LocomotorSurfaceTypeMask = u32;

/// Coordinate helper functions

// Trait definitions for object system interfaces

/// Thing trait (matching C++ Thing base class)
pub trait Thing: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn get_object_id(&self) -> Option<ObjectID> {
        None
    }
    fn get_template(&self) -> Option<&dyn ThingTemplate>;
    fn get_position(&self) -> &Coord3D;
    fn set_position(&mut self, pos: &Coord3D);
    fn get_angle(&self) -> Real;
    fn set_angle(&mut self, angle: Real);
}

/// Snapshot trait for serialization (matching C++ Snapshot)
pub trait Snapshot {
    fn crc(&self, xfer: &mut dyn Xfer);
    fn xfer(&mut self, xfer: &mut dyn Xfer);
    fn load_post_process(&mut self);
}

/// Thing template interface trait
pub trait ThingTemplate: Any + AsAny + Send + Sync + std::fmt::Debug {
    fn get_name(&self) -> &AsciiString;
    fn get_template_geometry_info(&self) -> GeometryInfo;
    fn get_template_geometry_type(&self) -> Option<EngineGeometryType> {
        None
    }
    fn calc_vision_range(&self) -> Real;
    fn calc_shroud_clearing_range(&self) -> Real;
    fn is_kind_of(&self, kind: KindOf) -> bool;
    fn is_enter_guard(&self) -> bool {
        false
    }
    fn is_hijack_guard(&self) -> bool {
        false
    }
    fn is_build_facility(&self) -> bool {
        false
    }

    /// Get the unique ID for this template
    /// Stub implementation - returns 0 by default
    fn get_id(&self) -> u32 {
        0
    }
    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        &[]
    }
    fn get_build_cost(&self) -> Int {
        0
    }
    fn get_experience_value(&self, _level: usize) -> Int {
        0
    }
    fn get_experience_required(&self, _level: usize) -> Int {
        0
    }
    fn is_trainable(&self) -> bool {
        false
    }
    /// Base build time in seconds (matches ThingTemplate::getBuildTime).
    fn get_build_time(&self) -> Real {
        0.0
    }
    /// C++ ThingTemplate::getThreatValue().
    fn get_threat_value(&self) -> UnsignedInt {
        0
    }
    /// C++ ThingTemplate::getShroudRevealToAllRange().
    fn get_shroud_reveal_to_all_range(&self) -> Real {
        0.0
    }
    /// Check if this template is equivalent to another template
    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        self.get_name() == other.get_name()
    }

    fn get_initial_object_status(&self) -> ObjectStatusMaskType {
        ObjectStatusMaskType::none()
    }

    fn get_model_name(&self) -> &str {
        self.get_name()
    }

    /// Command set string associated with this template (used by the control bar).
    fn get_command_set_string(&self) -> &AsciiString {
        static EMPTY: OnceLock<AsciiString> = OnceLock::new();
        EMPTY.get_or_init(AsciiString::new)
    }

    fn module_descriptors(&self) -> ModuleDescriptorSet {
        ModuleDescriptorSet::default()
    }

    fn get_draw_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    fn get_client_update_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    /// Behavior module descriptors (mirrors C++ ThingTemplate)
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    /// Maximum health for objects using this template (C++ ThingTemplate::GetMaxHealth)
    fn get_max_health(&self) -> Real {
        0.0
    }

    /// Whether this template supplies physics data
    fn has_physics(&self) -> bool {
        false
    }

    /// Default radar priority for this template.
    /// C++ Reference: ThingTemplate::getDefaultRadarPriority()
    fn get_radar_priority(&self) -> RadarPriorityType {
        RadarPriorityType::Invalid
    }

    /// Crushing power rating for this template.
    /// C++ Reference: ThingTemplate::getCrusherLevel()
    fn get_crusher_level(&self) -> u32 {
        0
    }

    /// Vulnerability to being crushed for this template.
    /// C++ Reference: ThingTemplate::getCrushableLevel()
    fn get_crushable_level(&self) -> u32 {
        255
    }

    /// Initial physics type
    fn get_physics_type(&self) -> PhysicsType {
        PhysicsType::Normal
    }

    /// Mass for physics simulation
    fn get_mass(&self) -> Real {
        0.0
    }

    /// Initial transform for spawned objects
    fn get_initial_transform(&self) -> Matrix3D {
        Matrix3D::IDENTITY
    }

    /// Get occlusion delay in frames.
    /// Returns 0 by default (templates with occlusion data should override).
    fn get_occlusion_delay(&self) -> u32 {
        0
    }

    /// Calculate cost to build with player modifiers.
    /// Uses player modifiers when a Player is supplied.
    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let base_cost = self.get_build_cost();
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return base_cost;
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_cost_change_percent =
            player.get_production_cost_change_percent(self.get_name().as_str());
        mods.handicap_cost_multiplier = player.get_handicap().get_cost_multiplier();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(calc_kind_of_mask(self));

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_cost_to_build(base_cost, &mods)
    }

    /// Energy production/consumption for this template (negative = consumption).
    fn get_energy_production(&self) -> Int {
        0
    }

    /// Extra energy bonus granted by upgrades (e.g., reactor).
    fn get_energy_bonus(&self) -> Int {
        0
    }

    /// Calculate time to build in frames with player modifiers.
    /// Defaults to build time * frames per second, clamped to 0 when no player is supplied.
    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let base_time = self.get_build_time();
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            let frames = (base_time * crate::common::LOGICFRAMES_PER_SECOND as f32).round() as Int;
            return frames.max(0);
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_time_change_percent =
            player.get_production_time_change_percent(self.get_name().as_str());
        mods.handicap_time_multiplier = player.get_handicap().get_build_time_multiplier();
        mods.energy_supply_ratio = player.get_energy().supply_ratio();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(calc_kind_of_mask(self));
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        {
            mods.builds_instantly = player.builds_instantly();
        }

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_time_to_build(base_time, &mods, None) as Int
    }

    /// Optional rubble height for structures (0 = use default).
    fn structure_rubble_height(&self) -> Option<u8> {
        None
    }

    /// Per-unit sound lookup (matches ThingTemplate::getPerUnitSound).
    fn get_per_unit_sound(&self, _name: &str) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient loop sound for the template.
    fn get_sound_ambient(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient damaged loop sound for the template.
    fn get_sound_ambient_damaged(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient really-damaged loop sound for the template.
    fn get_sound_ambient_really_damaged(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient rubble loop sound for the template.
    fn get_sound_ambient_rubble(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Voice select sound (matches ThingTemplate::getVoiceSelect / TTAUDIO_voiceSelect).
    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice group select sound (matches ThingTemplate::getVoiceGroupSelect / TTAUDIO_voiceGroupSelect).
    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice move sound (matches ThingTemplate::getVoiceMove / TTAUDIO_voiceMove).
    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack sound (matches ThingTemplate::getVoiceAttack / TTAUDIO_voiceAttack).
    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice enter sound (matches ThingTemplate::getVoiceEnter / TTAUDIO_voiceEnter).
    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice fear sound (matches ThingTemplate::getVoiceFear / TTAUDIO_voiceFear).
    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice select elite sound (matches ThingTemplate::getVoiceSelectElite / TTAUDIO_voiceSelectElite).
    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice created sound (matches ThingTemplate::getVoiceCreated / TTAUDIO_voiceCreated).
    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice task unable sound (matches ThingTemplate::getVoiceTaskUnable / TTAUDIO_voiceTaskUnable).
    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice task complete sound (matches ThingTemplate::getVoiceTaskComplete / TTAUDIO_voiceTaskComplete).
    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice meet enemy sound (matches ThingTemplate::getVoiceMeetEnemy / TTAUDIO_voiceMeetEnemy).
    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice garrison sound (matches ThingTemplate::getVoiceGarrison / TTAUDIO_voiceGarrison).
    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice defect sound (matches ThingTemplate::getVoiceDefect / TTAUDIO_voiceDefect).
    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack special sound (matches ThingTemplate::getVoiceAttackSpecial / TTAUDIO_voiceAttackSpecial).
    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack air sound (matches ThingTemplate::getVoiceAttackAir / TTAUDIO_voiceAttackAir).
    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice guard sound (matches ThingTemplate::getVoiceGuard / TTAUDIO_voiceGuard).
    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move start sound (matches ThingTemplate::getSoundMoveStart / TTAUDIO_soundMoveStart).
    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move start damaged sound (matches ThingTemplate::getSoundMoveStartDamaged / TTAUDIO_soundMoveStartDamaged).
    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move loop sound (matches ThingTemplate::getSoundMoveLoop / TTAUDIO_soundMoveLoop).
    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move loop damaged sound (matches ThingTemplate::getSoundMoveLoopDamaged / TTAUDIO_soundMoveLoopDamaged).
    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Stealth on sound (matches ThingTemplate::getSoundStealthOn / TTAUDIO_soundStealthOn).
    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Stealth off sound (matches ThingTemplate::getSoundStealthOff / TTAUDIO_soundStealthOff).
    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound created (matches ThingTemplate::getSoundCreated / TTAUDIO_soundCreated).
    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound on damaged (matches ThingTemplate::getSoundOnDamaged / TTAUDIO_soundOnDamaged).
    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound on really damaged (matches ThingTemplate::getSoundOnReallyDamaged / TTAUDIO_soundOnReallyDamaged).
    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound enter (matches ThingTemplate::getSoundEnter / TTAUDIO_soundEnter).
    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound exit (matches ThingTemplate::getSoundExit / TTAUDIO_soundExit).
    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted veteran (matches ThingTemplate::getSoundPromotedVeteran / TTAUDIO_soundPromotedVeteran).
    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted elite (matches ThingTemplate::getSoundPromotedElite / TTAUDIO_soundPromotedElite).
    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted hero (matches ThingTemplate::getSoundPromotedHero / TTAUDIO_soundPromotedHero).
    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound falling from plane (matches ThingTemplate::getSoundFalling / TTAUDIO_soundFalling).
    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Per-unit FX lookup (matches ThingTemplate::getPerUnitFX).
    fn get_per_unit_fx(&self, _name: &str) -> Option<crate::common::audio::AudioEventRts> {
        None
    }
}

fn calc_kind_of_mask<T: ThingTemplate + ?Sized>(template: &T) -> KindOfMaskType {
    let mut mask: KindOfMaskType = KIND_OF_MASK_NONE;
    for &kind in ALL_KIND_OF {
        if template.is_kind_of(kind) {
            mask |= 1u64 << (kind as u32);
        }
    }
    mask
}

/// Default thing template implementation
#[derive(Debug, Clone)]
pub struct DefaultThingTemplate {
    name: AsciiString,
    geometry_info: GeometryInfo,
    vision_range: Real,
    shroud_clearing_range: Real,
    kind_of_flags: Vec<KindOf>,
    build_cost: Int,
    build_time: Real,
    threat_value: UnsignedInt,
    shroud_reveal_to_all_range: Real,
    occlusion_delay: u32,
    max_health: Real,
    energy_production: Int,
    energy_bonus: Int,
    command_set_string: AsciiString,
    armor_template_sets: Vec<ArmorTemplateSet>,

    // --- Audio: per-unit sounds / FX (key = condition name) ---
    per_unit_sounds: HashMap<String, crate::common::audio::AudioEventRts>,
    per_unit_fx: HashMap<String, crate::common::audio::AudioEventRts>,

    // --- Audio: voice events (TTAUDIO_voice*) ---
    voice_select: crate::common::audio::AudioEventRts,
    voice_group_select: crate::common::audio::AudioEventRts,
    voice_move: crate::common::audio::AudioEventRts,
    voice_attack: crate::common::audio::AudioEventRts,
    voice_enter: crate::common::audio::AudioEventRts,
    voice_fear: crate::common::audio::AudioEventRts,
    voice_select_elite: crate::common::audio::AudioEventRts,
    voice_created: crate::common::audio::AudioEventRts,
    voice_task_unable: crate::common::audio::AudioEventRts,
    voice_task_complete: crate::common::audio::AudioEventRts,
    voice_meet_enemy: crate::common::audio::AudioEventRts,
    voice_garrison: crate::common::audio::AudioEventRts,
    voice_defect: crate::common::audio::AudioEventRts,
    voice_attack_special: crate::common::audio::AudioEventRts,
    voice_attack_air: crate::common::audio::AudioEventRts,
    voice_guard: crate::common::audio::AudioEventRts,

    // --- Audio: sound events (TTAUDIO_sound*) ---
    sound_move_start: crate::common::audio::AudioEventRts,
    sound_move_start_damaged: crate::common::audio::AudioEventRts,
    sound_move_loop: crate::common::audio::AudioEventRts,
    sound_move_loop_damaged: crate::common::audio::AudioEventRts,
    sound_ambient: crate::common::audio::AudioEventRts,
    sound_ambient_damaged: crate::common::audio::AudioEventRts,
    sound_ambient_really_damaged: crate::common::audio::AudioEventRts,
    sound_ambient_rubble: crate::common::audio::AudioEventRts,
    sound_stealth_on: crate::common::audio::AudioEventRts,
    sound_stealth_off: crate::common::audio::AudioEventRts,
    sound_created: crate::common::audio::AudioEventRts,
    sound_on_damaged: crate::common::audio::AudioEventRts,
    sound_on_really_damaged: crate::common::audio::AudioEventRts,
    sound_enter: crate::common::audio::AudioEventRts,
    sound_exit: crate::common::audio::AudioEventRts,
    sound_promoted_veteran: crate::common::audio::AudioEventRts,
    sound_promoted_elite: crate::common::audio::AudioEventRts,
    sound_promoted_hero: crate::common::audio::AudioEventRts,
    sound_falling: crate::common::audio::AudioEventRts,
}

impl DefaultThingTemplate {
    pub fn new(name: String) -> Self {
        let audio_default = crate::common::audio::AudioEventRts::default;
        Self {
            name: AsciiString::from(&name),
            geometry_info: GeometryInfo::default(),
            vision_range: 100.0,
            shroud_clearing_range: -1.0,
            kind_of_flags: Vec::new(),
            build_cost: 0,
            build_time: 0.0,
            threat_value: 0,
            shroud_reveal_to_all_range: 0.0,
            occlusion_delay: global_data::read().default_occlusion_delay,
            max_health: 100.0,
            energy_production: 0,
            energy_bonus: 0,
            command_set_string: AsciiString::new(),
            armor_template_sets: Vec::new(),
            per_unit_sounds: HashMap::new(),
            per_unit_fx: HashMap::new(),
            // Voice events
            voice_select: audio_default(),
            voice_group_select: audio_default(),
            voice_move: audio_default(),
            voice_attack: audio_default(),
            voice_enter: audio_default(),
            voice_fear: audio_default(),
            voice_select_elite: audio_default(),
            voice_created: audio_default(),
            voice_task_unable: audio_default(),
            voice_task_complete: audio_default(),
            voice_meet_enemy: audio_default(),
            voice_garrison: audio_default(),
            voice_defect: audio_default(),
            voice_attack_special: audio_default(),
            voice_attack_air: audio_default(),
            voice_guard: audio_default(),
            // Sound events
            sound_move_start: audio_default(),
            sound_move_start_damaged: audio_default(),
            sound_move_loop: audio_default(),
            sound_move_loop_damaged: audio_default(),
            sound_ambient: audio_default(),
            sound_ambient_damaged: audio_default(),
            sound_ambient_really_damaged: audio_default(),
            sound_ambient_rubble: audio_default(),
            sound_stealth_on: audio_default(),
            sound_stealth_off: audio_default(),
            sound_created: audio_default(),
            sound_on_damaged: audio_default(),
            sound_on_really_damaged: audio_default(),
            sound_enter: audio_default(),
            sound_exit: audio_default(),
            sound_promoted_veteran: audio_default(),
            sound_promoted_elite: audio_default(),
            sound_promoted_hero: audio_default(),
            sound_falling: audio_default(),
        }
    }

    pub fn set_max_health(&mut self, max_health: Real) {
        self.max_health = max_health.max(0.0);
    }

    pub fn set_build_time(&mut self, build_time: Real) {
        self.build_time = build_time.max(0.0);
    }

    pub fn set_threat_value(&mut self, threat_value: UnsignedInt) {
        self.threat_value = threat_value;
    }

    pub fn set_shroud_reveal_to_all_range(&mut self, range: Real) {
        self.shroud_reveal_to_all_range = range.max(0.0);
    }

    pub fn set_occlusion_delay(&mut self, delay: u32) {
        self.occlusion_delay = delay;
    }

    pub fn set_energy_production(&mut self, energy: Int) {
        self.energy_production = energy;
    }

    pub fn set_energy_bonus(&mut self, bonus: Int) {
        self.energy_bonus = bonus;
    }

    pub fn add_armor_template_set(&mut self, set: ArmorTemplateSet) {
        self.armor_template_sets.push(set);
    }

    pub fn set_per_unit_sound(
        &mut self,
        name: impl Into<String>,
        sound: crate::common::audio::AudioEventRts,
    ) {
        self.per_unit_sounds.insert(name.into(), sound);
    }

    pub fn set_per_unit_fx(
        &mut self,
        name: impl Into<String>,
        fx: crate::common::audio::AudioEventRts,
    ) {
        self.per_unit_fx.insert(name.into(), fx);
    }

    pub fn set_voice_attack(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack = sound;
    }

    pub fn set_voice_attack_special(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack_special = sound;
    }

    pub fn set_voice_attack_air(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack_air = sound;
    }

    /// Helper: set a voice field from an INI string value.
    fn set_voice_from_ini(&mut self, field: &str, value: &str) {
        let audio = crate::common::audio::AudioEventRts::new(value);
        match field {
            "VoiceSelect" => self.voice_select = audio,
            "VoiceGroupSelect" => self.voice_group_select = audio,
            "VoiceMove" => self.voice_move = audio,
            "VoiceAttack" => self.voice_attack = audio,
            "VoiceEnter" => self.voice_enter = audio,
            "VoiceFear" => self.voice_fear = audio,
            "VoiceSelectElite" => self.voice_select_elite = audio,
            "VoiceCreated" => self.voice_created = audio,
            "VoiceTaskUnable" => self.voice_task_unable = audio,
            "VoiceTaskComplete" => self.voice_task_complete = audio,
            "VoiceMeetEnemy" => self.voice_meet_enemy = audio,
            "VoiceGarrison" => self.voice_garrison = audio,
            "VoiceDefect" => self.voice_defect = audio,
            "VoiceAttackSpecial" => self.voice_attack_special = audio,
            "VoiceAttackAir" => self.voice_attack_air = audio,
            "VoiceGuard" => self.voice_guard = audio,
            _ => {}
        }
    }

    /// Helper: set a sound field from an INI string value.
    fn set_sound_from_ini(&mut self, field: &str, value: &str) {
        let audio = crate::common::audio::AudioEventRts::new(value);
        match field {
            "SoundMoveStart" => self.sound_move_start = audio,
            "SoundMoveStartDamaged" => self.sound_move_start_damaged = audio,
            "SoundMoveLoop" => self.sound_move_loop = audio,
            "SoundMoveLoopDamaged" => self.sound_move_loop_damaged = audio,
            "SoundAmbient" => self.sound_ambient = audio,
            "SoundAmbientDamaged" => self.sound_ambient_damaged = audio,
            "SoundAmbientReallyDamaged" => self.sound_ambient_really_damaged = audio,
            "SoundAmbientRubble" => self.sound_ambient_rubble = audio,
            "SoundStealthOn" => self.sound_stealth_on = audio,
            "SoundStealthOff" => self.sound_stealth_off = audio,
            "SoundCreated" => self.sound_created = audio,
            "SoundOnDamaged" => self.sound_on_damaged = audio,
            "SoundOnReallyDamaged" => self.sound_on_really_damaged = audio,
            "SoundEnter" => self.sound_enter = audio,
            "SoundExit" => self.sound_exit = audio,
            "SoundPromotedVeteran" => self.sound_promoted_veteran = audio,
            "SoundPromotedElite" => self.sound_promoted_elite = audio,
            "SoundPromotedHero" => self.sound_promoted_hero = audio,
            "SoundFallingFromPlane" => self.sound_falling = audio,
            _ => {}
        }
    }

    pub fn set_command_set_string(&mut self, command_set: AsciiString) {
        self.command_set_string = command_set;
    }

    fn kind_of_mask(&self) -> KindOfMaskType {
        let mut mask: KindOfMaskType = KIND_OF_MASK_NONE;
        for &kind in ALL_KIND_OF {
            if self.is_kind_of(kind) {
                mask |= 1u64 << (kind as u32);
            }
        }
        mask
    }

    pub fn find_armor_template_set(&self, flags: &ArmorSetBitFlags) -> Option<&ArmorTemplateSet> {
        self.armor_template_sets
            .iter()
            .find(|set| set.types() == flags)
            .or_else(|| self.armor_template_sets.first())
    }

    /// Apply parsed INI key=value properties to this template.
    ///
    /// This is the GameLogic-side equivalent of `parse_object_fields_from_ini`
    /// in the Common crate ThingTemplate.  It handles fields that are specific
    /// to the GameLogic layer (KindOf, MaxHealth, Armor, etc.) and delegates
    /// the rest to the Common-layer parsing.
    ///
    /// C++ Reference: ThingTemplate::s_objectFieldParseTable (ThingTemplate.cpp:90-229)
    pub fn parse_object_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) {
        for (key, value) in properties {
            let trimmed = value.trim();
            match key.as_str() {
                // --- Vision / shroud ---
                "VisionRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.vision_range = v;
                    }
                }
                "ShroudClearingRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_clearing_range = v;
                    }
                }
                "ShroudRevealToAllRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_reveal_to_all_range = v;
                    }
                }

                // --- Build ---
                "BuildCost" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.build_cost = v;
                    }
                }
                "BuildTime" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.build_time = v;
                    }
                }

                // --- Combat ---
                "ThreatValue" => {
                    if let Ok(v) = trimmed.parse::<UnsignedInt>() {
                        self.threat_value = v;
                    }
                }

                // --- Occlusion ---
                "OcclusionDelay" => {
                    if let Ok(v) = trimmed.parse::<u32>() {
                        self.occlusion_delay = v;
                    }
                }

                // --- Energy ---
                "EnergyProduction" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.energy_production = v;
                    }
                }
                "EnergyBonus" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.energy_bonus = v;
                    }
                }

                // --- Command set ---
                "CommandSet" => {
                    self.command_set_string = AsciiString::from(trimmed);
                }

                // --- KindOf ---
                "KindOf" => {
                    // Parse pipe-separated KindOf flags
                    // C++ uses KindOfMaskType::parseFromINI
                    self.kind_of_flags.clear();
                    for token in trimmed.split('|') {
                        let t = token.trim();
                        if t.is_empty() {
                            continue;
                        }
                        if let Some(kind) = kindof_from_name(t) {
                            self.kind_of_flags.push(kind);
                        }
                    }
                }

                // --- Voice events (C++ TTAUDIO_voice*) ---
                // Parsed via INI::parseDynamicAudioEventRTS in C++
                key @ "VoiceSelect"
                | key @ "VoiceGroupSelect"
                | key @ "VoiceMove"
                | key @ "VoiceAttack"
                | key @ "VoiceEnter"
                | key @ "VoiceFear"
                | key @ "VoiceSelectElite"
                | key @ "VoiceCreated"
                | key @ "VoiceTaskUnable"
                | key @ "VoiceTaskComplete"
                | key @ "VoiceMeetEnemy"
                | key @ "VoiceGarrison"
                | key @ "VoiceDefect"
                | key @ "VoiceAttackSpecial"
                | key @ "VoiceAttackAir"
                | key @ "VoiceGuard" => {
                    self.set_voice_from_ini(key, trimmed);
                }

                // --- Sound events (C++ TTAUDIO_sound*) ---
                // Parsed via INI::parseDynamicAudioEventRTS in C++
                key @ "SoundMoveStart"
                | key @ "SoundMoveStartDamaged"
                | key @ "SoundMoveLoop"
                | key @ "SoundMoveLoopDamaged"
                | key @ "SoundAmbient"
                | key @ "SoundAmbientDamaged"
                | key @ "SoundAmbientReallyDamaged"
                | key @ "SoundAmbientRubble"
                | key @ "SoundStealthOn"
                | key @ "SoundStealthOff"
                | key @ "SoundCreated"
                | key @ "SoundOnDamaged"
                | key @ "SoundOnReallyDamaged"
                | key @ "SoundEnter"
                | key @ "SoundExit"
                | key @ "SoundPromotedVeteran"
                | key @ "SoundPromotedElite"
                | key @ "SoundPromotedHero"
                | key @ "SoundFallingFromPlane" => {
                    self.set_sound_from_ini(key, trimmed);
                }

                // --- ArmorSet sub-blocks are handled separately ---
                // --- UnitSpecificSounds / UnitSpecificFX sub-blocks are handled separately ---

                // Everything else is silently ignored
                _ => {}
            }
        }
    }
}

impl Default for DefaultThingTemplate {
    fn default() -> Self {
        Self::new("DefaultThing".to_string())
    }
}

impl ThingTemplate for DefaultThingTemplate {
    fn get_name(&self) -> &AsciiString {
        &self.name
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.geometry_info.clone()
    }

    fn calc_vision_range(&self) -> Real {
        self.vision_range
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        self.shroud_clearing_range
    }

    fn get_command_set_string(&self) -> &AsciiString {
        &self.command_set_string
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        self.per_unit_sounds.get(name).cloned()
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack.clone()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack_special.clone()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack_air.clone()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of_flags.contains(&kind)
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        &[]
    }

    fn get_build_cost(&self) -> Int {
        self.build_cost
    }

    fn get_occlusion_delay(&self) -> u32 {
        self.occlusion_delay
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return self.get_build_cost();
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_cost_change_percent =
            player.get_production_cost_change_percent(self.get_name().as_str());
        mods.handicap_cost_multiplier = player.get_handicap().get_cost_multiplier();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kind_of_mask());

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_cost_to_build(self.get_build_cost(), &mods)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return (self.get_build_time() * crate::common::LOGICFRAMES_PER_SECOND as f32).round()
                as Int;
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_time_change_percent =
            player.get_production_time_change_percent(self.get_name().as_str());
        mods.handicap_time_multiplier = player.get_handicap().get_build_time_multiplier();
        mods.energy_supply_ratio = player.get_energy().supply_ratio();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kind_of_mask());
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        {
            mods.builds_instantly = player.builds_instantly();
        }

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_time_to_build(self.get_build_time(), &mods, None) as Int
    }

    fn get_build_time(&self) -> Real {
        self.build_time
    }

    fn get_threat_value(&self) -> UnsignedInt {
        self.threat_value
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        self.shroud_reveal_to_all_range
    }

    fn get_max_health(&self) -> Real {
        self.max_health
    }

    fn get_energy_production(&self) -> Int {
        self.energy_production
    }

    fn get_energy_bonus(&self) -> Int {
        self.energy_bonus
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        self.per_unit_fx.get(name).cloned()
    }

    // --- Voice events ---
    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        self.voice_select.clone()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        self.voice_group_select.clone()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        self.voice_move.clone()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        self.voice_enter.clone()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        self.voice_fear.clone()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        self.voice_select_elite.clone()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        self.voice_created.clone()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        self.voice_task_unable.clone()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        self.voice_task_complete.clone()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        self.voice_meet_enemy.clone()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        self.voice_garrison.clone()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        self.voice_defect.clone()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        self.voice_guard.clone()
    }

    // --- Sound events ---
    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_start.clone()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_start_damaged.clone()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_loop.clone()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_loop_damaged.clone()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        self.sound_stealth_on.clone()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        self.sound_stealth_off.clone()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        self.sound_created.clone()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_on_damaged.clone()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_on_really_damaged.clone()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        self.sound_enter.clone()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        self.sound_exit.clone()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_veteran.clone()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_elite.clone()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_hero.clone()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        self.sound_falling.clone()
    }
}

// // Implement ThingTemplate for Arc<DefaultThingTemplate> to support Arc-wrapped types
impl ThingTemplate for Arc<DefaultThingTemplate> {
    fn get_name(&self) -> &AsciiString {
        (**self).get_name()
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        (**self).get_template_geometry_info()
    }

    fn calc_vision_range(&self) -> Real {
        (**self).calc_vision_range()
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        (**self).calc_shroud_clearing_range()
    }

    fn get_command_set_string(&self) -> &AsciiString {
        (**self).get_command_set_string()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        (**self).is_kind_of(kind)
    }

    fn is_enter_guard(&self) -> bool {
        (**self).is_enter_guard()
    }

    fn is_hijack_guard(&self) -> bool {
        (**self).is_hijack_guard()
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        (**self).weapon_template_sets()
    }

    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        (**self).is_equivalent_to(other)
    }

    fn get_build_cost(&self) -> Int {
        (**self).get_build_cost()
    }

    fn get_experience_value(&self, level: usize) -> Int {
        (**self).get_experience_value(level)
    }

    fn get_experience_required(&self, level: usize) -> Int {
        (**self).get_experience_required(level)
    }

    fn is_trainable(&self) -> bool {
        (**self).is_trainable()
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_cost_to_build(player)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_time_to_build(player)
    }

    fn get_build_time(&self) -> Real {
        (**self).get_build_time()
    }

    fn get_threat_value(&self) -> UnsignedInt {
        (**self).get_threat_value()
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        (**self).get_shroud_reveal_to_all_range()
    }

    fn get_occlusion_delay(&self) -> u32 {
        (**self).get_occlusion_delay()
    }

    fn get_energy_production(&self) -> Int {
        (**self).get_energy_production()
    }

    fn get_energy_bonus(&self) -> Int {
        (**self).get_energy_bonus()
    }

    fn structure_rubble_height(&self) -> Option<u8> {
        (**self).structure_rubble_height()
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_sound(name)
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_fx(name)
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_special()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_air()
    }

    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_group_select()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_move()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_enter()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_fear()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select_elite()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_created()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_unable()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_complete()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_meet_enemy()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_garrison()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_defect()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_guard()
    }

    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start_damaged()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop_damaged()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_on()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_off()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_created()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_damaged()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_really_damaged()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_enter()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_exit()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_veteran()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_elite()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_hero()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_falling()
    }
}

// Implement ThingTemplate for Arc<dyn ThingTemplate> to support trait object Arc wrapping
impl ThingTemplate for Arc<dyn ThingTemplate> {
    fn get_name(&self) -> &AsciiString {
        (**self).get_name()
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        (**self).get_template_geometry_info()
    }

    fn calc_vision_range(&self) -> Real {
        (**self).calc_vision_range()
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        (**self).calc_shroud_clearing_range()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        (**self).is_kind_of(kind)
    }

    fn is_enter_guard(&self) -> bool {
        (**self).is_enter_guard()
    }

    fn is_hijack_guard(&self) -> bool {
        (**self).is_hijack_guard()
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        (**self).weapon_template_sets()
    }

    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        (**self).is_equivalent_to(other)
    }

    fn get_build_cost(&self) -> Int {
        (**self).get_build_cost()
    }

    fn get_experience_value(&self, level: usize) -> Int {
        (**self).get_experience_value(level)
    }

    fn get_experience_required(&self, level: usize) -> Int {
        (**self).get_experience_required(level)
    }

    fn is_trainable(&self) -> bool {
        (**self).is_trainable()
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_cost_to_build(player)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_time_to_build(player)
    }

    fn get_build_time(&self) -> Real {
        (**self).get_build_time()
    }

    fn get_threat_value(&self) -> UnsignedInt {
        (**self).get_threat_value()
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        (**self).get_shroud_reveal_to_all_range()
    }

    fn get_energy_production(&self) -> Int {
        (**self).get_energy_production()
    }

    fn get_energy_bonus(&self) -> Int {
        (**self).get_energy_bonus()
    }

    fn structure_rubble_height(&self) -> Option<u8> {
        (**self).structure_rubble_height()
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_sound(name)
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_fx(name)
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_special()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_air()
    }

    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_group_select()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_move()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_enter()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_fear()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select_elite()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_created()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_unable()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_complete()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_meet_enemy()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_garrison()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_defect()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_guard()
    }

    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start_damaged()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop_damaged()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_on()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_off()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_created()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_damaged()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_really_damaged()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_enter()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_exit()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_veteran()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_elite()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_hero()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_falling()
    }
}

// Utility functions

const SELECTABLE_KIND_INDICES: &[u32] = &[1];
const UNIT_KIND_INDICES: &[u32] = &[8, 9, 10, 11, 12, 13, 19, 20, 72, 81, 89];
const STRUCTURE_KIND_INDICES: &[u32] = &[
    7, 22, 23, 24, 37, 61, 62, 63, 64, 93, 94, 95, 96, 97, 102, 103, 108, 109, 110, 111,
];
const VEHICLE_KIND_INDICES: &[u32] = &[9, 11, 12, 13, 21];
const HARVESTER_KIND_INDICES: &[u32] = &[13];
const AIRCRAFT_KIND_INDICES: &[u32] = &[10, 110, 111];
const DRONE_KIND_INDICES: &[u32] = &[72];
const CRATE_KIND_INDICES: &[u32] = &[48];
const RESOURCE_NODE_KIND_INDICES: &[u32] = &[85];
const SUPPLY_SOURCE_ON_PREVIEW_KIND_INDICES: &[u32] = &[76];
const SUPPLY_SOURCE_KIND_INDICES: &[u32] = &[85];
const DISGUISER_KIND_INDICES: &[u32] = &[87];
const TECH_BUILDING_KIND_INDICES: &[u32] = &[69];
const BRIDGE_KIND_INDICES: &[u32] = &[22, 23, 24];
const WALL_KIND_INDICES: &[u32] = &[60];
const SALVAGER_KIND_INDICES: &[u32] = &[19];
const WEAPON_SALVAGER_KIND_INDICES: &[u32] = &[20];
const ARMOR_SALVAGER_KIND_INDICES: &[u32] = &[99];
const ALWAYS_SELECTABLE_INDICES: &[u32] = &[57];
const CAN_ATTACK_KIND_INDICES: &[u32] = &[3];
const PROJECTILE_KIND_INDICES: &[u32] = &[25];
const CLIFF_JUMPER_INDICES: &[u32] = &[92];
const PRISON_KIND_INDICES: &[u32] = &[15];
const COLLECTS_PRISON_BOUNTY_INDICES: &[u32] = &[16];
const POW_TRUCK_KIND_INDICES: &[u32] = &[17];
const CAN_SURRENDER_KIND_INDICES: &[u32] = &[44];

fn engine_kind_indices(kind: KindOf) -> &'static [u32] {
    match kind {
        KindOf::Selectable | KindOf::AlwaysSelectable => SELECTABLE_KIND_INDICES,
        KindOf::Unit => UNIT_KIND_INDICES,
        KindOf::Building | KindOf::Structure | KindOf::KeyStructure | KindOf::CommandCenter => {
            STRUCTURE_KIND_INDICES
        }
        KindOf::Vehicle | KindOf::Dozer | KindOf::Hulk => VEHICLE_KIND_INDICES,
        KindOf::Harvester => HARVESTER_KIND_INDICES,
        KindOf::Infantry | KindOf::Saboteur | KindOf::Hacker => &[8],
        KindOf::Aircraft | KindOf::AircraftCarrier => AIRCRAFT_KIND_INDICES,
        KindOf::Drone => DRONE_KIND_INDICES,
        KindOf::CliffJumper => CLIFF_JUMPER_INDICES,
        KindOf::Weapon => CAN_ATTACK_KIND_INDICES,
        KindOf::Projectile => PROJECTILE_KIND_INDICES,
        KindOf::Crate => CRATE_KIND_INDICES,
        KindOf::ResourceNode => RESOURCE_NODE_KIND_INDICES,
        KindOf::SupplySourceOnPreview => SUPPLY_SOURCE_ON_PREVIEW_KIND_INDICES,
        KindOf::SupplySource => SUPPLY_SOURCE_KIND_INDICES,
        KindOf::Disguiser => DISGUISER_KIND_INDICES,
        KindOf::TechBuilding => TECH_BUILDING_KIND_INDICES,
        KindOf::Bridge => BRIDGE_KIND_INDICES,
        KindOf::Barrier => WALL_KIND_INDICES,
        KindOf::Shrubbery => &[6],
        KindOf::CanSeeThrough => &[73],
        KindOf::CanCrossBridges => BRIDGE_KIND_INDICES,
        KindOf::BridgeTower => BRIDGE_KIND_INDICES,
        KindOf::WaveGuide => &[],
        KindOf::Boat => &[79],
        KindOf::Salvager => SALVAGER_KIND_INDICES,
        KindOf::WeaponSalvager => WEAPON_SALVAGER_KIND_INDICES,
        KindOf::ArmorSalvager => ARMOR_SALVAGER_KIND_INDICES,
        KindOf::FSBarracks => &[108],
        KindOf::FSWarfactory => &[109],
        KindOf::FSAirfield => &[110],
        KindOf::FSInternetCenter => &[103],
        KindOf::FSPower | KindOf::PowerPlant => &[61],
        KindOf::FSSupplyDropzone => &[93],
        KindOf::FSSupplyCenter | KindOf::Refinery => &[96],
        KindOf::FSSuperweapon => &[94],
        KindOf::FSStrategyCenter => &[97],
        KindOf::FSFake => &[102],
        KindOf::Defense => &[63, 115],
        KindOf::Factory => &[62],
        KindOf::Mine => &[],
        KindOf::Prison => PRISON_KIND_INDICES,
        KindOf::CollectsPrisonBounty => COLLECTS_PRISON_BOUNTY_INDICES,
        KindOf::PowTruck => POW_TRUCK_KIND_INDICES,
        KindOf::CanSurrender => CAN_SURRENDER_KIND_INDICES,
        KindOf::Civilian
        | KindOf::Destructible
        | KindOf::Amphibious
        | KindOf::AmphibiousTransport
        | KindOf::CanCapture
        | KindOf::Hero
        | KindOf::CountsForVictory
        | KindOf::CleanupHazard
        | KindOf::Immobile
        | KindOf::BoobyTrap
        | KindOf::CanBeRepulsed
        | KindOf::EmpHardened
        | KindOf::SpawnsAreTheWeapons => &[],
        KindOf::Obstacle => &[0],
        KindOf::CanAttack => &[3],
        KindOf::StickToTerrainSlope
        | KindOf::CanCastReflections
        | KindOf::Preload
        | KindOf::NoCollide
        | KindOf::StealthGarrison
        | KindOf::DrawableOnly
        | KindOf::Score
        | KindOf::ScoreCreate
        | KindOf::ScoreDestroy
        | KindOf::NoHealIcon
        | KindOf::Parachutable
        | KindOf::AlwaysVisible
        | KindOf::Unattackable
        | KindOf::AircraftPathAround
        | KindOf::LowOverlappable
        | KindOf::ForceAttackable
        | KindOf::AutoRallypoint
        | KindOf::ClickThrough
        | KindOf::ShowPortraitWhenControlled
        | KindOf::CannotBuildNearSupplies
        | KindOf::RevealToAll
        | KindOf::IgnoresSelectAll
        | KindOf::DontAutoCrushInfantry
        | KindOf::FsBlackMarket
        | KindOf::FsAdvancedTech
        | KindOf::RevealsEnemyPaths
        | KindOf::NoSelect
        | KindOf::CannotRetaliate
        | KindOf::Demotrap
        | KindOf::ConservativeBuilding
        | KindOf::BlastCrater
        | KindOf::Prop
        | KindOf::OptimizedTree
        | KindOf::WaveEffect
        | KindOf::ClearedByBuild => &[],
        KindOf::HugeVehicle => &[11],
        KindOf::LineBuild => &[60],
        KindOf::SmallMissile | KindOf::BallisticMissile => &[25],
        KindOf::AttackNeedsLineOfSight => &[3],
        KindOf::WalkOnTopOfWall | KindOf::DefensiveWall => &[60],
        KindOf::MoneyHacker => &[87],
        KindOf::TechBaseDefense => &[69],
        KindOf::LandmarkBridge => &[22, 23, 24],
        _ => &[],
    }
}

pub fn kind_of_indices(kind: KindOf) -> &'static [u32] {
    engine_kind_indices(kind)
}

fn engine_geometry_to_logic(info: &EngineGeometryInfo) -> GeometryInfo {
    let half_width = info.width * 0.5;
    let half_depth = info.depth * 0.5;
    let height = info.height.max(0.0);

    // Approximate bounds centered at origin
    let min = Coord3D::new(-half_width, -half_depth, 0.0);
    let max = Coord3D::new(half_width, half_depth, height);

    GeometryInfo {
        position: Coord3D::new(0.0, 0.0, if info.is_small { 0.0 } else { height * 0.5 }),
        angle: 0.0,
        bounds: AABox { min, max },
        height_above_terrain: if matches!(info.geometry_type, EngineGeometryType::Sphere) {
            0.0
        } else {
            height
        },
        geometry_type: info.geometry_type,
        is_small: info.is_small,
    }
}

/// Test disabled mask function (matching C++ TEST_DISABLEDMASK)
pub fn test_disabled_mask(mask: DisabledMaskType, disabled_type: DisabledType) -> bool {
    mask.test(disabled_type)
}

// Module interfaces that the Object system needs

/// Dummy Xfer implementation for now
pub struct DummyXfer;

impl Xfer for DummyXfer {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Invalid
    }

    fn get_identifier(&self) -> &str {
        ""
    }

    fn set_options(&mut self, _options: u32) {}

    fn clear_options(&mut self, _options: u32) {}

    fn get_options(&self) -> u32 {
        0
    }

    fn open(&mut self, _identifier: &str) -> Result<(), XferStatus> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        Ok(0)
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn skip(&mut self, _data_size: i32) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer_snapshot(
        &mut self,
        _snapshot: &mut game_engine::system::Snapshot,
    ) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer_ascii_string(&mut self, _ascii_string_data: &mut String) -> io::Result<()> {
        Ok(())
    }

    fn xfer_unicode_string(&mut self, _unicode_string_data: &mut String) -> io::Result<()> {
        Ok(())
    }

    unsafe fn xfer_implementation(&mut self, _data: *mut u8, _data_size: usize) -> io::Result<()> {
        let _ = (_data, _data_size); // Silence unused warning
        Ok(())
    }
}

// Partition manager traits (matching C++ partition system)

/// Base partition manager trait for spatial partitioning
pub trait PartitionManager: Send + Sync {
    /// Get objects within radius of a point
    fn get_objects_in_radius(&self, _pos: &Coord3D, _radius: Real) -> Vec<ObjectID> {
        Vec::new() // Default implementation
    }

    /// Reveal map for a specific player
    fn reveal_map_for_player(
        &self,
        _player_id: PlayerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(()) // Default implementation
    }
}

/// Unit-specific partition manager
pub trait UnitPartitionManager: PartitionManager {
    /// Get units within radius of a point
    fn get_units_in_radius(&self, pos: &Coord3D, radius: Real) -> Vec<ObjectID>;

    /// Find a legal position around a point
    fn find_position_around(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Options for finding positions around objects
#[derive(Debug, Clone)]
pub struct FindPositionOptions {
    pub min_radius: Real,
    pub max_radius: Real,
    pub start_angle: Option<Real>,
    pub max_z_delta: Real,
    pub flags: u32,
    pub relationship_object_id: Option<ObjectID>,
    pub ignore_object_id: Option<ObjectID>,
    pub source_to_path_to_dest_id: Option<ObjectID>,
}

impl Default for FindPositionOptions {
    fn default() -> Self {
        Self {
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: None,
            max_z_delta: 99999.0,
            flags: 0,
            relationship_object_id: None,
            ignore_object_id: None,
            source_to_path_to_dest_id: None,
        }
    }
}

// Missing types that are referenced in various modules
/// Drawable ID for referencing drawable objects
pub type DrawableID = u32;

/// Wide character type (UTF-16)
pub type WideChar = u16;

/// Unicode string type
pub type UnicodeString = std::string::String;

/// Kind of mask type for object classification (matches C++ KindOfMaskType)
pub type KindOfMaskType = u64;

/// Alias without Type suffix (matches C++ usage)
pub type KindOfMask = KindOfMaskType;

/// Bitmask with all KindOf flags enabled.
pub const KIND_OF_MASK_ALL: KindOfMaskType = u64::MAX;
/// Bitmask with no KindOf flags enabled.
pub const KIND_OF_MASK_NONE: KindOfMaskType = 0;

// Additional missing types found during compilation

/// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        GameDifficulty::Medium
    }
}

/// Area type for geographical regions
#[derive(Debug, Clone)]
pub struct Area {
    pub name: String,
    pub boundary: Region3D,
    pub properties: HashMap<String, String>,
}

// Send + Sync implementations for structs that need thread safety
unsafe impl Send for GeometryInfo {}
unsafe impl Sync for GeometryInfo {}

unsafe impl Send for AABox {}
unsafe impl Sync for AABox {}

unsafe impl Send for DefaultThingTemplate {}
unsafe impl Sync for DefaultThingTemplate {}

unsafe impl Send for FindPositionOptions {}
unsafe impl Sync for FindPositionOptions {}

unsafe impl Send for Area {}
unsafe impl Sync for Area {}

impl Area {
    pub fn new(name: String, boundary: Region3D) -> Self {
        Self {
            name,
            boundary,
            properties: HashMap::new(),
        }
    }
}

/// Terrain type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Sand,
    Rock,
    Water,
    Cliff,
    Beach,
    Road,
    // Add more as needed
}

impl Default for TerrainType {
    fn default() -> Self {
        TerrainType::Grass
    }
}

// DamageTypeFlags is defined in src/damage.rs and re-exported via common module
// Use crate::damage::DamageTypeFlags or crate::common::DamageTypeFlags
// Helper functions for coordinate types (replaces removed impl blocks)

/// Coordinate helper functions
pub mod coord_helpers {
    use super::*;

    pub fn coord3d_zero() -> Coord3D {
        Coord3D::origin()
    }

    pub fn coord2d_zero() -> Coord2D {
        Coord2D::origin()
    }

    pub fn icoord2d_zero() -> ICoord2D {
        ICoord2D::origin()
    }

    pub fn icoord2d_new(x: i32, y: i32) -> ICoord2D {
        ICoord2D::new(x, y)
    }

    pub fn icoord3d_zero() -> ICoord3D {
        ICoord3D::origin()
    }

    pub fn icoord3d_new(x: i32, y: i32, z: i32) -> ICoord3D {
        ICoord3D::new(x, y, z)
    }
}

/// Update context for object update modules
///
/// This context provides access to game subsystems needed by update modules.
/// It matches the pattern used in AIUpdateContext for AI modules.
///
/// # Fields
///
/// * `game_logic` - Reference to the GameLogic system for object queries and game state
/// * `terrain_logic` - Reference to terrain system for height queries and edge detection
/// * `object_creation_list` - System for creating new objects via OCLs
/// * `partition_manager` - Spatial partitioning for distance and proximity queries
/// * `particle_system_manager` - (Optional) Particle system management for visual effects
/// * `control_bar` - (Optional) Control bar interface for command buttons and command sets
/// * `thing_factory` - (Optional) Thing factory for creating objects from templates
/// * `upgrade_center` - (Optional) Upgrade center for managing upgrades
/// * `weapon_store` - (Optional) Weapon store for weapon template lookups
/// * `game_client` - (Optional) Game client interface for drawables and rendering
/// * `fx_list` - (Optional) FX list manager for special effects
/// * `audio` - (Optional) Audio interface for sound management
#[derive(Debug)]
pub struct UpdateContext<'a> {
    /// Reference to the main GameLogic system
    pub game_logic: &'a mut dyn GameLogicInterface,

    /// Reference to terrain system
    pub terrain_logic: &'a dyn TerrainLogicInterface,

    /// Reference to object creation list system
    pub object_creation_list: &'a mut dyn ObjectCreationListInterface,

    /// Reference to partition manager for spatial queries
    pub partition_manager: &'a dyn PartitionManagerInterface,

    /// Reference to particle system manager for visual effects (optional)
    pub particle_system_manager: Option<&'a dyn ParticleSystemManagerInterface>,

    /// Reference to control bar for command buttons (optional)
    pub control_bar: Option<&'a dyn ControlBarInterface>,

    /// Reference to thing factory for object creation (optional)
    pub thing_factory: Option<&'a dyn ThingFactoryInterface>,

    /// Reference to upgrade center for upgrade management (optional)
    pub upgrade_center: Option<&'a dyn UpgradeCenterInterface>,

    /// Reference to weapon store for weapon template lookups (optional)
    pub weapon_store: Option<&'a mut dyn WeaponStoreInterface>,

    /// Reference to game client for drawables and rendering (optional)
    pub game_client: Option<&'a dyn GameClientInterface>,

    /// Reference to FX list manager for special effects (optional)
    pub fx_list: Option<&'a dyn FXListManagerInterface>,

    /// Reference to object creation list manager for creating objects (optional)
    pub object_creation_list_manager: Option<&'a mut dyn ObjectCreationListInterface>,

    /// Reference to FX list manager for special effects (optional)
    pub fx_list_manager: Option<&'a dyn FXListManagerInterface>,

    /// Reference to audio system for sound management (optional)
    pub audio: Option<&'a mut dyn AudioInterface>,

    /// Reference to build assistant for construction management (optional)
    pub build_assistant: Option<&'a dyn BuildAssistantInterface>,
}

/// Trait for GameLogic interface used by UpdateContext
///
/// This allows update modules to access game logic functionality without
/// tight coupling to the concrete GameLogic implementation.
pub trait GameLogicInterface: std::fmt::Debug {
    /// Find an object by its ID
    fn find_object(&self, id: ThingId) -> Option<&Object>;

    /// Find a mutable object by its ID
    fn find_object_mut(&mut self, id: ThingId) -> Option<&mut Object>;

    /// Get the current game frame
    fn get_frame(&self) -> u32;

    /// Destroy an object
    fn destroy_object(&mut self, id: ThingId);
}

/// Trait for terrain logic interface used by UpdateContext
pub trait TerrainLogicInterface: std::fmt::Debug {
    /// Get ground height at a given position
    fn get_ground_height(&self, x: f32, y: f32) -> f32;

    /// Find closest edge point on the map
    fn find_closest_edge_point(&self, position: &Coord3D) -> Coord3D;
}

/// Trait for object creation list interface used by UpdateContext
pub trait ObjectCreationListInterface: std::fmt::Debug {
    /// Create objects from an OCL (Object Creation List)
    fn create(
        &mut self,
        ocl_id: ObjectCreationListId,
        source_object: Option<&Object>,
        position: &Coord3D,
        source_position: &Coord3D,
        orientation: f32,
    );
}

/// Trait for partition manager interface used by UpdateContext
pub trait PartitionManagerInterface: std::fmt::Debug {
    /// Get distance squared between two objects or points
    fn get_distance_squared(
        &self,
        a: &Object,
        b: &Object,
        distance_type: PartitionDistanceType,
    ) -> f32;

    /// Get distance squared between an object and a position
    fn get_distance_squared_to_pos(
        &self,
        obj: &Object,
        pos: &Coord3D,
        distance_type: PartitionDistanceType,
    ) -> f32;

    /// Get closest object matching filters
    fn get_closest_object(
        &self,
        from: &Object,
        max_range: f32,
        distance_type: PartitionDistanceType,
        filters: &[PartitionFilter],
    ) -> Option<Arc<RwLock<Object>>>;
}

/// Distance type for partition manager queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionDistanceType {
    /// Distance from center to center
    Center2D,
    /// Distance from bounding sphere edge to edge
    FromBoundingSphere2D,
    /// 3D distance
    Center3D,
}

/// Filter type for partition manager queries
/// Matches C++ PartitionFilter enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionFilter {
    /// Filter for flammable objects
    Flammable,
    /// Filter for enemy objects
    Enemy,
    /// Filter for friendly objects
    Friendly,
    /// Filter for neutral objects
    Neutral,
    /// Filter for targetable objects
    Targetable,
    /// Filter for attackable objects
    Attackable,
    /// Filter for objects that can heal
    CanHeal,
    /// Filter for objects that can repair
    CanRepair,
    /// Filter for objects with specific kindof
    KindOf(KindOf),
}

/// Constant for 3D center distance (uses PartitionDistanceType enum)
pub const PARTITION_FROM_CENTER_3D: PartitionDistanceType = PartitionDistanceType::Center3D;

/// Radius decal for visual effects
/// Matches C++ RadiusDecal class
#[derive(Debug, Clone)]
pub struct RadiusDecal {
    /// Position in world space
    pub position: Coord3D,
    /// Radius of the decal
    pub radius: f32,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Color of the decal
    pub color: u32,
    /// Minimum opacity for throb effects
    pub min_opacity: f32,
    /// Maximum opacity for throb effects
    pub max_opacity: f32,
    /// Opacity throb time (frames)
    pub opacity_throb_time: u32,
    /// Template that created this decal
    pub template: Option<RadiusDecalTemplateId>,
}

impl RadiusDecal {
    /// Create a new radius decal
    pub fn new(position: Coord3D, radius: f32) -> Self {
        Self {
            position,
            radius,
            opacity: 1.0,
            color: 0xFFFFFFFF,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            template: None,
        }
    }

    /// Set decal opacity (0.0 to 1.0).
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    /// Set decal position.
    pub fn set_position(&mut self, position: Coord3D) {
        self.position = position;
    }

    /// Returns true if decal is effectively empty.
    pub fn is_empty(&self) -> bool {
        self.radius <= 0.0
    }

    /// Update throb opacity using current game frame.
    /// Matches C++ RadiusDecal::update behavior including draw-icon visibility gating.
    pub fn update(&mut self) {
        let draw_icon_ui = crate::helpers::TheGameLogic::get_draw_icon_ui();
        self.update_with_draw_icon_ui(draw_icon_ui);
    }

    /// Deterministic update helper that allows direct visibility control in tests/callers.
    pub fn update_with_draw_icon_ui(&mut self, draw_icon_ui: bool) {
        if !draw_icon_ui {
            self.opacity = 0.0;
            return;
        }

        if self.opacity_throb_time == 0 {
            self.opacity = self.max_opacity;
            return;
        }

        let now = crate::helpers::TheGameLogic::get_frame();
        let theta = (2.0 * std::f32::consts::PI)
            * ((now % self.opacity_throb_time) as f32 / self.opacity_throb_time as f32);
        let percent = 0.5 * (theta.sin() + 1.0);
        let lo = self.min_opacity.min(self.max_opacity);
        let hi = self.min_opacity.max(self.max_opacity);
        self.opacity = lo + percent * (hi - lo);
    }

    /// Reset the decal to an empty state (matches C++ RadiusDecal::clear).
    pub fn clear(&mut self) {
        self.position = Coord3D::origin();
        self.radius = 0.0;
        self.opacity = 1.0;
        self.color = 0xFFFFFFFF;
        self.min_opacity = 1.0;
        self.max_opacity = 1.0;
        self.opacity_throb_time = LOGICFRAMES_PER_SECOND;
        self.template = None;
    }
}

#[cfg(test)]
mod radius_decal_tests {
    use super::{Coord3D, CoordOrigin, RadiusDecal};

    #[test]
    fn radius_decal_update_hides_when_draw_icon_ui_disabled() {
        let mut decal = RadiusDecal::new(Coord3D::origin(), 10.0);
        decal.min_opacity = 0.2;
        decal.max_opacity = 0.9;
        decal.update_with_draw_icon_ui(false);
        assert_eq!(decal.opacity, 0.0);
    }

    #[test]
    fn radius_decal_update_uses_max_when_throb_time_is_zero() {
        let mut decal = RadiusDecal::new(Coord3D::origin(), 10.0);
        decal.min_opacity = 0.1;
        decal.max_opacity = 0.8;
        decal.opacity_throb_time = 0;
        decal.update_with_draw_icon_ui(true);
        assert!((decal.opacity - 0.8).abs() < f32::EPSILON);
    }
}

/// ID type for radius decal templates
pub type RadiusDecalTemplateId = u32;

// Shadow type bit flags (matches GameClient/Shadow.h TheShadowNames order)
pub const SHADOW_DECAL: u32 = 0x0000_0001;
pub const SHADOW_VOLUME: u32 = 0x0000_0002;
pub const SHADOW_PROJECTION: u32 = 0x0000_0004;
pub const SHADOW_DYNAMIC_PROJECTION: u32 = 0x0000_0008;
pub const SHADOW_DIRECTIONAL_PROJECTION: u32 = 0x0000_0010;
pub const SHADOW_ALPHA_DECAL: u32 = 0x0000_0020;
pub const SHADOW_ADDITIVE_DECAL: u32 = 0x0000_0040;

pub const SHADOW_NAMES: [&str; 7] = [
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

/// Template for radius decals
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplate {
    /// Default radius
    pub radius: f32,
    /// Default opacity
    pub opacity: f32,
    /// Default color
    pub color: u32,
    /// Texture name (if applicable)
    pub texture_name: AsciiString,
    /// Shadow/decal style flags (bitset, matches ShadowType)
    pub shadow_type: u32,
    /// Minimum opacity for throb effects
    pub min_opacity: f32,
    /// Maximum opacity for throb effects
    pub max_opacity: f32,
    /// Opacity throb time (frames)
    pub opacity_throb_time: u32,
    /// Visibility restricted to owning player
    pub only_visible_to_owning_player: bool,
}

impl Default for RadiusDecalTemplate {
    fn default() -> Self {
        Self {
            radius: 0.0,
            opacity: 1.0,
            color: 0,
            texture_name: AsciiString::new(),
            shadow_type: SHADOW_ALPHA_DECAL,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            only_visible_to_owning_player: true,
        }
    }
}

impl RadiusDecalTemplate {
    /// Create a radius decal from this template
    pub fn create_radius_decal(&self, position: Coord3D) -> RadiusDecal {
        if self.texture_name.is_empty() || self.radius <= 0.0 {
            return RadiusDecal::new(Coord3D::origin(), 0.0);
        }

        RadiusDecal {
            position,
            radius: self.radius,
            opacity: self.max_opacity,
            color: self.color,
            min_opacity: self.min_opacity,
            max_opacity: self.max_opacity,
            opacity_throb_time: self.opacity_throb_time,
            template: None,
        }
    }

    /// Create a radius decal using an explicit radius (matches C++ createRadiusDecal parameter).
    pub fn create_radius_decal_with_radius(&self, position: Coord3D, radius: f32) -> RadiusDecal {
        if self.texture_name.is_empty() || radius <= 0.0 {
            return RadiusDecal::new(Coord3D::origin(), 0.0);
        }

        RadiusDecal {
            position,
            radius,
            opacity: self.max_opacity,
            color: self.color,
            min_opacity: self.min_opacity,
            max_opacity: self.max_opacity,
            opacity_throb_time: self.opacity_throb_time,
            template: None,
        }
    }
}

/// Particle emission volume type (mirrors C++ EmissionVolumeType, subset used by gameplay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVolumeType {
    None,
    Sphere,
    Cylinder,
}

impl Default for EmissionVolumeType {
    fn default() -> Self {
        EmissionVolumeType::None
    }
}

/// Trait for particle system manager interface used by UpdateContext
pub trait ParticleSystemManagerInterface: std::fmt::Debug + Send + Sync {
    /// Find a particle system template by name
    fn find_template(&self, name: &str) -> Option<ParticleSystemTemplateId>;

    /// Create a particle system from a template
    fn create_particle_system(
        &self,
        template_id: ParticleSystemTemplateId,
    ) -> Option<ParticleSystemId>;

    /// Create an attached particle system and return its ID
    fn create_attached_particle_system_id(
        &self,
        template_id: ParticleSystemTemplateId,
        object_id: ObjectID,
    ) -> Option<ParticleSystemId>;

    /// Find a particle system by ID
    fn find_particle_system(&self, system_id: ParticleSystemId) -> Option<Box<dyn std::any::Any>>;

    /// Set particle system position (mirrors ParticleSystem::setPosition)
    fn set_particle_system_position(&self, system_id: ParticleSystemId, position: &Coord3D);

    /// Get particle system position (mirrors ParticleSystem::getPosition)
    fn get_particle_system_position(&self, system_id: ParticleSystemId) -> Option<Coord3D>;

    /// Attach particle system to an object (mirrors ParticleSystem::attachToObject)
    fn attach_particle_system_to_object(&self, system_id: ParticleSystemId, object_id: ObjectID);

    /// Attach particle system to a drawable (mirrors ParticleSystem::attachToDrawable)
    fn attach_particle_system_to_drawable(
        &self,
        system_id: ParticleSystemId,
        drawable_id: ObjectID,
    );

    /// Set particle system local transform (mirrors ParticleSystem::setLocalTransform)
    fn set_particle_system_transform(&self, system_id: ParticleSystemId, transform: &Matrix3D);

    /// Destroy a particle system by ID
    fn destroy_particle_system(&self, system_id: ParticleSystemId);

    /// Get emission volume type for a particle system
    fn get_particle_system_emission_volume_type(
        &self,
        system_id: ParticleSystemId,
    ) -> Option<EmissionVolumeType>;

    /// Set emission volume sphere radius for a particle system
    fn set_particle_system_emission_volume_sphere_radius(
        &self,
        system_id: ParticleSystemId,
        radius: Real,
    );

    /// Set emission volume cylinder radius for a particle system
    fn set_particle_system_emission_volume_cylinder_radius(
        &self,
        system_id: ParticleSystemId,
        radius: Real,
    );

    /// Start emitting particles from a system (mirrors ParticleSystem::start).
    fn start_particle_system(&self, _system_id: ParticleSystemId) {}

    /// Stop emitting new particles from a system (mirrors ParticleSystem::stop).
    fn stop_particle_system(&self, _system_id: ParticleSystemId) {}

    /// Tint all active particle-system colors (mirrors ParticleSystem::tintAllColors).
    fn tint_particle_system_all_colors(&self, _system_id: ParticleSystemId, _color: Color) {}

    /// Scale particle velocity on an active system (mirrors ParticleSystem::setVelocityMultiplier).
    fn set_particle_system_velocity_multiplier(
        &self,
        _system_id: ParticleSystemId,
        _multiplier: &Coord3D,
    ) {
    }

    /// Scale burst count on an active system (mirrors ParticleSystem::setBurstCountMultiplier).
    fn set_particle_system_burst_count_multiplier(
        &self,
        _system_id: ParticleSystemId,
        _multiplier: Real,
    ) {
    }

    /// Destroy all particle systems attached to the given object (mirrors ParticleSystemManager::destroyAttachedSystems).
    fn destroy_attached_systems(&self, _object_id: ObjectID) {}
}

/// Trait for control bar interface used by UpdateContext
pub trait ControlBarInterface: std::fmt::Debug + Send + Sync {
    /// Find a command set by name
    fn find_command_set(&self, name: &str) -> Option<&dyn std::any::Any>;

    /// Get a command button by ID
    fn get_command_button(&self, button_id: CommandButtonId) -> Option<&dyn std::any::Any>;
}

/// Trait for thing factory interface used by UpdateContext
pub trait ThingFactoryInterface: std::fmt::Debug + Send + Sync {
    /// Find a template by name
    fn find_template(&self, name: &str) -> Option<Arc<dyn ThingTemplate>>;

    /// Get a template by ID
    fn get_template(&self, template_id: u32) -> Option<Arc<dyn ThingTemplate>>;

    /// Create a new object from a template
    fn new_object(
        &self,
        template: Arc<dyn ThingTemplate>,
        team: &dyn std::any::Any,
    ) -> Result<Arc<dyn std::any::Any>, String>;
}

/// Trait for upgrade center interface used by UpdateContext
pub trait UpgradeCenterInterface: std::fmt::Debug + Send + Sync {
    /// Check if a player can afford an upgrade
    fn can_afford_upgrade(&self, player: &dyn std::any::Any, upgrade: &dyn std::any::Any) -> bool;

    /// Find an upgrade by ID
    fn find_upgrade(&self, upgrade_id: u32) -> Option<&dyn std::any::Any>;
}

/// Trait for weapon store interface used by UpdateContext
pub trait WeaponStoreInterface: std::fmt::Debug + Send + Sync {
    /// Find a weapon template by name
    fn find_weapon_template(&self, name: &str) -> Option<&dyn std::any::Any>;

    /// Get a weapon template by ID
    fn get_weapon_template(&self, template_id: WeaponTemplateId) -> Option<&dyn std::any::Any>;

    /// Allocate a new weapon instance from a template
    fn allocate_new_weapon(
        &mut self,
        template_id: WeaponTemplateId,
        slot_type: WeaponSlotType,
    ) -> WeaponId {
        // Default implementation returns invalid ID
        let _ = (template_id, slot_type);
        0
    }

    /// Get a weapon by ID (immutable)
    fn get_weapon(&self, weapon_id: WeaponId) -> Option<&dyn std::any::Any> {
        // Default implementation returns None
        let _ = weapon_id;
        None
    }

    /// Get a mutable weapon by ID
    fn get_weapon_mut(&mut self, weapon_id: WeaponId) -> Option<&mut dyn std::any::Any> {
        // Default implementation returns None
        let _ = weapon_id;
        None
    }
}

/// Trait for game client interface used by UpdateContext
/// Provides access to client-side rendering and drawable systems
pub trait GameClientInterface: std::fmt::Debug + Send + Sync {
    /// Find a drawable by its ID
    fn find_drawable_by_id(&self, id: DrawableId) -> Option<&dyn std::any::Any>;
}

/// Trait for FX list manager interface used by UpdateContext
/// Manages special effects execution
pub trait FXListManagerInterface: std::fmt::Debug + Send + Sync {
    /// Execute FX at a position
    fn do_fx_pos(&self, fx_list: FXListId, position: &Coord3D, matrix: Option<&Mat4>);

    /// Execute FX on an object
    fn do_fx_obj(&self, fx_list: FXListId, object_id: ThingId);

    /// Execute FX on an object with an optional source object for orientation.
    fn do_fx_obj_with_source(
        &self,
        fx_list: FXListId,
        object_id: ThingId,
        _source_id: Option<ThingId>,
    ) {
        self.do_fx_obj(fx_list, object_id);
    }
}

/// Trait for audio interface used by UpdateContext
/// Manages game audio events
pub trait AudioInterface: std::fmt::Debug + Send + Sync {
    /// Add an audio event and return its handle
    fn add_audio_event(&mut self, event: &dyn std::any::Any) -> u32;

    /// Remove an audio event by handle
    fn remove_audio_event(&mut self, handle: u32);
}

/// Build assistant interface for construction validation
pub trait BuildAssistantInterface: std::fmt::Debug + Send + Sync {
    /// Check if a unit can be made (including prerequisites and money check)
    fn can_make_unit(
        &self,
        builder: &dyn std::any::Any,
        what_to_build: &dyn ThingTemplate,
    ) -> crate::object::update::production_update::CanMakeType;
}

impl<'a> UpdateContext<'a> {
    /// Create a new update context with only the required core interfaces.
    ///
    /// Optional interfaces (particle_system_manager, control_bar, thing_factory,
    /// upgrade_center, weapon_store, game_client, fx_list, object_creation_list_manager,
    /// fx_list_manager, audio) are set to None by default.
    /// Use the builder methods (with_*) to add them as needed.
    pub fn new(
        game_logic: &'a mut dyn GameLogicInterface,
        terrain_logic: &'a dyn TerrainLogicInterface,
        object_creation_list: &'a mut dyn ObjectCreationListInterface,
        partition_manager: &'a dyn PartitionManagerInterface,
    ) -> Self {
        Self {
            game_logic,
            terrain_logic,
            object_creation_list,
            partition_manager,
            particle_system_manager: None,
            control_bar: None,
            thing_factory: None,
            upgrade_center: None,
            weapon_store: None,
            game_client: None,
            fx_list: None,
            object_creation_list_manager: None,
            fx_list_manager: None,
            audio: None,
            build_assistant: None,
        }
    }

    /// Add particle system manager to the context (builder pattern)
    pub fn with_particle_system_manager(
        mut self,
        particle_system_manager: &'a dyn ParticleSystemManagerInterface,
    ) -> Self {
        self.particle_system_manager = Some(particle_system_manager);
        self
    }

    /// Add control bar to the context (builder pattern)
    pub fn with_control_bar(mut self, control_bar: &'a dyn ControlBarInterface) -> Self {
        self.control_bar = Some(control_bar);
        self
    }

    /// Add thing factory to the context (builder pattern)
    pub fn with_thing_factory(mut self, thing_factory: &'a dyn ThingFactoryInterface) -> Self {
        self.thing_factory = Some(thing_factory);
        self
    }

    /// Add upgrade center to the context (builder pattern)
    pub fn with_upgrade_center(mut self, upgrade_center: &'a dyn UpgradeCenterInterface) -> Self {
        self.upgrade_center = Some(upgrade_center);
        self
    }

    /// Add weapon store to the context (builder pattern)
    pub fn with_weapon_store(mut self, weapon_store: &'a mut dyn WeaponStoreInterface) -> Self {
        self.weapon_store = Some(weapon_store);
        self
    }

    /// Add game client to the context (builder pattern)
    pub fn with_game_client(mut self, game_client: &'a dyn GameClientInterface) -> Self {
        self.game_client = Some(game_client);
        self
    }

    /// Add FX list manager to the context (builder pattern)
    pub fn with_fx_list(mut self, fx_list: &'a dyn FXListManagerInterface) -> Self {
        self.fx_list = Some(fx_list);
        self
    }

    /// Add object creation list manager to the context (builder pattern)
    pub fn with_object_creation_list_manager(
        mut self,
        object_creation_list_manager: &'a mut dyn ObjectCreationListInterface,
    ) -> Self {
        self.object_creation_list_manager = Some(object_creation_list_manager);
        self
    }

    /// Add FX list manager to the context (builder pattern)
    pub fn with_fx_list_manager(mut self, fx_list_manager: &'a dyn FXListManagerInterface) -> Self {
        self.fx_list_manager = Some(fx_list_manager);
        self
    }

    /// Add audio system to the context (builder pattern)
    pub fn with_audio(mut self, audio: &'a mut dyn AudioInterface) -> Self {
        self.audio = Some(audio);
        self
    }

    /// Get particle system manager if available
    pub fn particle_system_manager(&self) -> Option<&dyn ParticleSystemManagerInterface> {
        self.particle_system_manager
    }

    /// Get control bar if available
    pub fn control_bar(&self) -> Option<&dyn ControlBarInterface> {
        self.control_bar
    }

    /// Get thing factory if available
    pub fn thing_factory(&self) -> Option<&dyn ThingFactoryInterface> {
        self.thing_factory
    }

    /// Get upgrade center if available
    pub fn upgrade_center(&self) -> Option<&dyn UpgradeCenterInterface> {
        self.upgrade_center
    }

    /// Get weapon store if available
    pub fn weapon_store(&self) -> Option<&dyn WeaponStoreInterface> {
        self.weapon_store
            .as_ref()
            .map(|ws| *ws as &dyn WeaponStoreInterface)
    }

    /// Get the current game frame number
    pub fn get_frame(&self) -> u32 {
        self.game_logic.get_frame()
    }

    /// Set the wake frame for an update module
    /// This schedules when the module should next be updated
    ///
    /// # Arguments
    /// * `object_id` - The object ID or thing ID to set wake frame for
    /// * `sleep_time` - When the module should wake up next
    pub fn set_wake_frame(
        &mut self,
        object_id: impl Into<ThingId>,
        sleep_time: crate::object::helper::UpdateSleepTime,
    ) {
        crate::helpers::TheGameLogic::set_wake_frame(object_id.into(), sleep_time);
    }
}

// ============================================================================
// Additional Type Aliases for C++ Compatibility
// ============================================================================

/// Object Creation List ID (matches C++ ObjectCreationListId)
pub type ObjectCreationListId = u32;

/// Particle System Template ID (matches C++ ParticleSystemTemplateId)
pub type ParticleSystemTemplateId = u32;

/// FX List ID (matches C++ FXListId)
pub type FXListId = u32;

/// Particle System ID (matches C++ ParticleSystemId)
pub type ParticleSystemId = u32;

/// Weapon Template ID (matches C++ WeaponTemplateId)
pub type WeaponTemplateId = u32;

/// Weapon ID (matches C++ WeaponId)
pub type WeaponId = u32;

/// Command Button ID (matches C++ CommandButtonId)
pub type CommandButtonId = u32;

/// Drawable ID (matches C++ DrawableId)
pub type DrawableId = u32;

/// Audio Handle (matches C++ AudioHandle)
pub type AudioHandle = u32;

/// Special Power Template ID (matches C++ SpecialPowerTemplateId)
pub type SpecialPowerTemplateId = u32;

/// Special Power Module ID (matches C++ SpecialPowerModuleId)
pub type SpecialPowerModuleId = u32;

/// Game Logic Context - provides access to game systems during updates
/// This is an alias to UpdateContext for backwards compatibility
pub type GameLogicContext<'a> = UpdateContext<'a>;

/// Turret Type enumeration (matches C++ TurretType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretType {
    Invalid = -1,
    Primary = 0,
    Secondary = 1,
}

/// Model Condition State - represents a snapshot of model conditions
/// Alias to ModelConditionFlags for convenience
pub type ModelConditionState = ModelConditionFlags;

// Command Options - bitflags for command execution options
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CommandOptions: u32 {
        const NONE = 0;
        const QUEUED = 1 << 0;
        const FORCE_ATTACK = 1 << 1;
        const FORCE_MOVE = 1 << 2;
        const ATTACK_MOVE = 1 << 3;
        const GUARD = 1 << 4;
        const FIRED_BY_SCRIPT = 0x0004_0000;
        const OPTION_ONE = 0x00002000;
        const OPTION_TWO = 0x00004000;
        const OPTION_THREE = 0x00008000;
    }
}

/// Random Variable - for randomized values in game logic
#[derive(Debug, Clone, Copy)]
pub struct RandomVariable {
    pub min: f32,
    pub max: f32,
}

impl RandomVariable {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn constant(value: f32) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    pub fn get_random_value(&self) -> f32 {
        if self.min == self.max {
            self.min
        } else {
            get_game_logic_random_value_real(self.min, self.max)
        }
    }

    /// Alias for get_random_value (matches C++ GetValue())
    pub fn get_value(&self) -> f32 {
        self.get_random_value()
    }
}

impl Default for RandomVariable {
    fn default() -> Self {
        Self { min: 0.0, max: 0.0 }
    }
}

/// AI Update trait - marker for AI update modules
pub trait AIUpdate: Send + Sync {
    fn update(
        &mut self,
        context: &mut UpdateContext<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Extended Radius Decal Template - template for radius-based decals with texture
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplateExt {
    pub texture: String,
    pub radius: f32,
    pub opacity_min: f32,
    pub opacity_max: f32,
}

impl Default for RadiusDecalTemplateExt {
    fn default() -> Self {
        Self {
            texture: String::new(),
            radius: 0.0,
            opacity_min: 1.0,
            opacity_max: 1.0,
        }
    }
}
