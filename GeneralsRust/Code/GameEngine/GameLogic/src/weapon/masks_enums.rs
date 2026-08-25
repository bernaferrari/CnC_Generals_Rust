//! Weapon masks, enums, and bonus types extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{INVALID_ID, ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion};
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
    get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

/// Weapon reload behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeaponReloadType {
    /// Weapon automatically reloads when clip is empty
    AutoReload,
    /// Weapon never reloads automatically
    NoReload,
    /// Unit must return to base to reload
    ReturnToBaseToReload,
}

/// INI names for weapon reload types (C++ TheWeaponReloadNames).
pub const WEAPON_RELOAD_NAMES: [&str; 3] = ["YES", "NO", "RETURN_TO_BASE"];

impl WeaponReloadType {
    pub fn from_ini(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "YES" => Some(Self::AutoReload),
            "NO" => Some(Self::NoReload),
            "RETURN_TO_BASE" => Some(Self::ReturnToBaseToReload),
            _ => None,
        }
    }

    pub fn as_ini_str(&self) -> &'static str {
        match self {
            Self::AutoReload => "YES",
            Self::NoReload => "NO",
            Self::ReturnToBaseToReload => "RETURN_TO_BASE",
        }
    }
}

/// Prefire delay behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeaponPrefireType {
    /// Use prefire delay for every shot
    PrefirePerShot,
    /// Use prefire delay each time attacking a new target
    PrefirePerAttack,
    /// Use prefire delay for each new clip
    PrefirePerClip,
}

/// INI names for weapon prefire types (C++ TheWeaponPrefireNames).
pub const WEAPON_PREFIRE_NAMES: [&str; 3] = ["PER_SHOT", "PER_ATTACK", "PER_CLIP"];
pub const WEAPON_PREFIRE_COUNT: usize = WEAPON_PREFIRE_NAMES.len();

impl WeaponPrefireType {
    pub fn from_ini(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "PER_SHOT" => Some(Self::PrefirePerShot),
            "PER_ATTACK" => Some(Self::PrefirePerAttack),
            "PER_CLIP" => Some(Self::PrefirePerClip),
            _ => None,
        }
    }

    pub fn as_ini_str(&self) -> &'static str {
        match self {
            Self::PrefirePerShot => "PER_SHOT",
            Self::PrefirePerAttack => "PER_ATTACK",
            Self::PrefirePerClip => "PER_CLIP",
        }
    }
}

/// INI names for weapon affects mask flags (C++ TheWeaponAffectsMaskNames).
pub const WEAPON_AFFECTS_MASK_NAMES: [&str; 7] = [
    "SELF",
    "ALLIES",
    "ENEMIES",
    "NEUTRALS",
    "SUICIDE",
    "NOT_SIMILAR",
    "NOT_AIRBORNE",
];

/// INI names for weapon collide mask flags (C++ TheWeaponCollideMaskNames).
pub const WEAPON_COLLIDE_MASK_NAMES: [&str; 9] = [
    "ALLIES",
    "ENEMIES",
    "STRUCTURES",
    "SHRUBBERY",
    "PROJECTILES",
    "WALLS",
    "SMALL_MISSILES",
    "BALLISTIC_MISSILES",
    "CONTROLLED_STRUCTURES",
];

/// INI names for weapon bonus conditions (C++ TheWeaponBonusNames).
pub const WEAPON_BONUS_NAMES: [&str; 27] = [
    "GARRISONED",
    "HORDE",
    "CONTINUOUS_FIRE_MEAN",
    "CONTINUOUS_FIRE_FAST",
    "NATIONALISM",
    "PLAYER_UPGRADE",
    "DRONE_SPOTTING",
    "DEMORALIZED",
    "ENTHUSIASTIC",
    "VETERAN",
    "ELITE",
    "HERO",
    "BATTLEPLAN_BOMBARDMENT",
    "BATTLEPLAN_HOLDTHELINE",
    "BATTLEPLAN_SEARCHANDDESTROY",
    "SUBLIMINAL",
    "SOLO_HUMAN_EASY",
    "SOLO_HUMAN_NORMAL",
    "SOLO_HUMAN_HARD",
    "SOLO_AI_EASY",
    "SOLO_AI_NORMAL",
    "SOLO_AI_HARD",
    "TARGET_FAERIE_FIRE",
    "FANATICISM",
    "FRENZY_ONE",
    "FRENZY_TWO",
    "FRENZY_THREE",
];

/// INI names for weapon bonus fields (C++ TheWeaponBonusFieldNames).
pub const WEAPON_BONUS_FIELD_NAMES: [&str; 5] =
    ["DAMAGE", "RADIUS", "RANGE", "RATE_OF_FIRE", "PRE_ATTACK"];

/// Weapon targeting anti-mask flags
#[derive(Debug, Clone, Copy)]
pub struct WeaponAntiMask(pub(super) u32);

impl WeaponAntiMask {
    pub const AIRBORNE_VEHICLE: u32 = 0x01;
    pub const GROUND: u32 = 0x02;
    pub const PROJECTILE: u32 = 0x04;
    pub const SMALL_MISSILE: u32 = 0x08;
    pub const MINE: u32 = 0x10;
    pub const AIRBORNE_INFANTRY: u32 = 0x20;
    pub const BALLISTIC_MISSILE: u32 = 0x40;
    pub const PARACHUTE: u32 = 0x80;

    pub fn new(mask: u32) -> Self {
        Self(mask)
    }

    pub fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    pub fn bits(&self) -> u32 {
        self.0
    }
}

/// Weapon affects mask flags
#[derive(Debug, Clone, Copy)]
pub struct WeaponAffectsMask(pub(super) u32);

impl WeaponAffectsMask {
    pub const SELF: u32 = 0x01;
    pub const ALLIES: u32 = 0x02;
    pub const ENEMIES: u32 = 0x04;
    pub const NEUTRALS: u32 = 0x08;
    pub const KILLS_SELF: u32 = 0x10;
    pub const DOESNT_AFFECT_SIMILAR: u32 = 0x20;
    pub const DOESNT_AFFECT_AIRBORNE: u32 = 0x40;

    pub fn new(mask: u32) -> Self {
        Self(mask)
    }

    pub fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    /// Get the raw bits of the mask
    pub fn bits(&self) -> u32 {
        self.0
    }
}

/// Weapon collision mask flags
#[derive(Debug, Clone, Copy)]
pub struct WeaponCollideMask(pub(super) u32);

impl WeaponCollideMask {
    pub const ALLIES: u32 = 0x0001;
    pub const ENEMIES: u32 = 0x0002;
    pub const STRUCTURES: u32 = 0x0004;
    pub const SHRUBBERY: u32 = 0x0008;
    pub const PROJECTILE: u32 = 0x0010;
    pub const WALLS: u32 = 0x0020;
    pub const SMALL_MISSILES: u32 = 0x0040;
    pub const BALLISTIC_MISSILES: u32 = 0x0080;
    pub const CONTROLLED_STRUCTURES: u32 = 0x0100;

    pub fn new(mask: u32) -> Self {
        Self(mask)
    }

    pub fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    pub fn bits(&self) -> u32 {
        self.0
    }
}

/// Weapon bonus condition types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WeaponBonusConditionType {
    Garrisoned = 0,
    Horde,
    ContinuousFireMean,
    ContinuousFireFast,
    Nationalism,
    PlayerUpgrade,
    DroneSpotting,
    Demoralized,
    Enthusiastic,
    Veteran,
    Elite,
    Hero,
    BattleplanBombardment,
    BattleplanHoldtheLine,
    BattleplanSearchAndDestroy,
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
}

pub const WEAPON_BONUS_CONDITION_COUNT: usize = WEAPON_BONUS_NAMES.len();

/// Weapon bonus condition flags
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponBonusConditionFlags(pub(super) u64);

impl WeaponBonusConditionFlags {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn empty() -> Self {
        Self::new()
    }

    pub fn set(&mut self, condition: WeaponBonusConditionType) {
        self.0 |= 1 << (condition as u8);
    }

    pub fn clear(&mut self, condition: WeaponBonusConditionType) {
        self.0 &= !(1 << (condition as u8));
    }

    pub fn has(&self, condition: WeaponBonusConditionType) -> bool {
        (self.0 & (1 << (condition as u8))) != 0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

/// Weapon bonus field types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeaponBonusField {
    Damage = 0,
    Radius,
    Range,
    RateOfFire,
    PreAttack,
}

pub const WEAPON_BONUS_FIELD_COUNT: usize = WEAPON_BONUS_FIELD_NAMES.len();

/// Weapon bonus multipliers
#[derive(Debug, Clone)]
pub struct WeaponBonus {
    fields: [f32; 5], // Maps to WeaponBonusField enum
}

impl WeaponBonus {
    pub fn new() -> Self {
        Self { fields: [1.0; 5] }
    }

    pub fn get_field(&self, field: WeaponBonusField) -> f32 {
        self.fields[field as usize]
    }

    pub fn set_field(&mut self, field: WeaponBonusField, value: f32) {
        self.fields[field as usize] = value;
    }

    pub fn clear(&mut self) {
        self.fields.fill(1.0);
    }

    /// C++ `WeaponBonus::appendBonuses` (Weapon.cpp:3463-3468) adds field
    /// deltas (`bonus += other - 1`) so stacked bonuses compose additively.
    pub fn append_bonuses(&mut self, other: &WeaponBonus) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            *field += other.fields[i] - 1.0;
        }
    }
}

impl Default for WeaponBonus {
    fn default() -> Self {
        Self::new()
    }
}

/// Weapon bonus set for different conditions
#[derive(Debug, Clone)]
pub struct WeaponBonusSet {
    bonuses: HashMap<WeaponBonusConditionType, WeaponBonus>,
}

impl WeaponBonusSet {
    pub fn new() -> Self {
        Self {
            bonuses: HashMap::new(),
        }
    }

    pub fn set_bonus(&mut self, condition: WeaponBonusConditionType, bonus: WeaponBonus) {
        self.bonuses.insert(condition, bonus);
    }

    pub fn get_bonus(&self, condition: WeaponBonusConditionType) -> Option<&WeaponBonus> {
        self.bonuses.get(&condition)
    }

    pub fn append_bonuses(&self, flags: WeaponBonusConditionFlags, bonus: &mut WeaponBonus) {
        for (&condition, weapon_bonus) in &self.bonuses {
            if flags.has(condition) {
                bonus.append_bonuses(weapon_bonus);
            }
        }
    }
}

impl Default for WeaponBonusSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Historic weapon damage tracking
#[derive(Debug, Clone)]
pub struct HistoricWeaponDamageInfo {
    pub frame: u32,
    pub location: Coord3D,
}

impl HistoricWeaponDamageInfo {
    pub fn new(frame: u32, location: Coord3D) -> Self {
        Self { frame, location }
    }
}

/// 2D coordinate for scatter targets
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Weapon slot types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary,
    Secondary,
    Tertiary,
}

/// Veterancy levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl From<ObjectStatusTypes> for crate::common::ObjectStatusTypes {
    fn from(value: ObjectStatusTypes) -> crate::common::ObjectStatusTypes {
        crate::common::ObjectStatusTypes::from_u32(value.0)
    }
}

impl From<crate::common::ObjectStatusTypes> for ObjectStatusTypes {
    fn from(value: crate::common::ObjectStatusTypes) -> Self {
        ObjectStatusTypes::new(value as u32)
    }
}

// damage_system now re-exports canonical types from crate::damage,
// so no From<> conversion impls needed between the two.

impl From<WeaponSlotType> for crate::common::WeaponSlotType {
    fn from(value: WeaponSlotType) -> Self {
        match value {
            WeaponSlotType::Primary => crate::common::WeaponSlotType::Primary,
            WeaponSlotType::Secondary => crate::common::WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary => crate::common::WeaponSlotType::Tertiary,
        }
    }
}

impl From<crate::common::WeaponSlotType> for WeaponSlotType {
    fn from(value: crate::common::WeaponSlotType) -> Self {
        match value {
            crate::common::WeaponSlotType::Primary => WeaponSlotType::Primary,
            crate::common::WeaponSlotType::Secondary => WeaponSlotType::Secondary,
            crate::common::WeaponSlotType::Tertiary => WeaponSlotType::Tertiary,
        }
    }
}

/// Object status types
#[derive(Debug, Clone, Copy)]
pub struct ObjectStatusTypes(pub(super) u32);

impl ObjectStatusTypes {
    pub const NONE: u32 = 0;

    pub fn new(status: u32) -> Self {
        Self(status)
    }
}

/// Weapon status — discriminants must match C++ WeaponStatus enum (WeaponStatus.h)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    ReadyToFire,        // C++ = 0
    OutOfAmmo,          // C++ = 1
    BetweenFiringShots, // C++ = 2
    ReloadingClip,      // C++ = 3
    PreAttack,          // C++ = 4
}

/// Weapon firing errors
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponError {
    NoAmmo,
    NotReady { time_remaining: f32 },
    OutOfRange { distance: f32, max_range: f32 },
    TargetObstructed,
    TargetNotVisible,
    InvalidTarget,
    NoTemplate,
    SystemError(String),
}

impl std::fmt::Display for WeaponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeaponError::NoAmmo => write!(f, "Weapon has no ammunition"),
            WeaponError::NotReady { time_remaining } => {
                write!(
                    f,
                    "Weapon not ready to fire ({:.2}s remaining)",
                    time_remaining
                )
            }
            WeaponError::OutOfRange {
                distance,
                max_range,
            } => {
                write!(
                    f,
                    "Target out of range ({:.1} > {:.1})",
                    distance, max_range
                )
            }
            WeaponError::TargetObstructed => write!(f, "Line of sight to target obstructed"),
            WeaponError::TargetNotVisible => write!(f, "Target is outside vision range"),
            WeaponError::InvalidTarget => write!(f, "Invalid or dead target"),
            WeaponError::NoTemplate => write!(f, "Weapon template not available"),
            WeaponError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for WeaponError {}

/// Fire mode for different weapon types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FireMode {
    /// Projectile weapon (creates physical projectile object)
    Projectile { speed: f32, lifetime: f32 },
    /// Instant impact weapon (no travel time)
    InstantImpact { splash_radius: f32 },
    /// Continuous beam weapon (sustained damage over time)
    ContinuousBeam {
        duration: f32,
        damage_per_frame: f32,
    },
}

/// Object type for scatter calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Infantry,
    Vehicle,
    Structure,
    Projectile,
    Unknown,
}
