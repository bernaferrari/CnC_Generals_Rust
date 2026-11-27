//! Weapon rules shared by both the live simulation and legacy templates.

use crate::pathfinding::PATHFIND_CELL_SIZE_F;

mod catalog;
mod definition;
mod estimate;
mod ini;
mod selection;
mod set_flags;
mod sets;
mod slot;

pub use crate::object::WeaponDamageStatus;
pub use catalog::{WeaponCatalog, WeaponCatalogError};
pub use definition::{Coord2D, WeaponDefinition, WeaponFxName, WeaponSound};
pub use estimate::{DamageEstimateTarget, WeaponDamageEstimate, estimate_weapon_template_damage};
pub use generals_game_domain::weapon::{WeaponFireStatus, WeaponLockType, WeaponSlotType};
pub use ini::{
    WeaponDefinitionLoadType, WeaponEffectOverrides, apply_weapon_ini, apply_weapon_ini_fields,
    parse_weapon_definitions_ini_into,
};
pub use selection::{
    WeaponCandidate, WeaponChoiceCriteria, WeaponSelection, weapon_damage_is_eligible,
};
pub use set_flags::{WeaponSetFlags, WeaponSetType};
pub use sets::{
    BoundWeaponSets, WeaponChoicePolicy, WeaponRow, WeaponRowIdentity, WeaponSlotDefinition,
    weapon_condition_mask,
};
pub use slot::{Weapon, WeaponTrackerBind};

/// Sentinel for an attack order without a maximum shot count.
pub const NO_MAX_SHOTS_LIMIT: i32 = i32::MAX;

const RANGE_UNDERSIZE: f32 = PATHFIND_CELL_SIZE_F * 0.25;

/// Minimum reach uses the same quarter-cell adjustment as maximum reach.
pub fn minimum_attack_range(range: f32) -> f32 {
    (range - RANGE_UNDERSIZE).max(0.0)
}

/// Contact weapons have an adjusted, unmodified reach below one cell.
pub fn is_contact_weapon(range: f32) -> bool {
    (range - RANGE_UNDERSIZE) < PATHFIND_CELL_SIZE_F
}

/// Choose the authored unmodified delay, drawing only for unequal bounds.
pub fn draw_shot_delay(minimum: i32, maximum: i32, random: impl FnOnce(i32, i32) -> i32) -> i32 {
    if minimum == maximum {
        minimum
    } else {
        random(minimum, maximum)
    }
}

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
pub struct WeaponAntiMask(u32);

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
pub struct WeaponAffectsMask(u32);

impl WeaponAffectsMask {
    pub const SELF: u32 = 0x01;
    pub const ALLIES: u32 = 0x02;
    pub const ENEMIES: u32 = 0x04;
    pub const NEUTRALS: u32 = 0x08;
    pub const KILLS_SELF: u32 = 0x10;
    pub const DOESNT_AFFECT_SIMILAR: u32 = 0x20;
    pub const DOESNT_AFFECT_AIRBORNE: u32 = 0x40;
    /// C++ WeaponTemplate default when RadiusDamageAffects is omitted.
    pub const DEFAULT: u32 = Self::ALLIES | Self::ENEMIES | Self::NEUTRALS;

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
pub struct WeaponCollideMask(u32);

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

impl WeaponBonusConditionType {
    /// Parse a named GameData bonus entry.
    pub fn from_ini(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "GARRISONED" => Some(Self::Garrisoned),
            "HORDE" => Some(Self::Horde),
            "CONTINUOUS_FIRE_MEAN" => Some(Self::ContinuousFireMean),
            "CONTINUOUS_FIRE_FAST" => Some(Self::ContinuousFireFast),
            "NATIONALISM" => Some(Self::Nationalism),
            "PLAYER_UPGRADE" => Some(Self::PlayerUpgrade),
            "DRONE_SPOTTING" => Some(Self::DroneSpotting),
            "DEMORALIZED" | "DEMORALIZED_OBSOLETE" => Some(Self::Demoralized),
            "ENTHUSIASTIC" => Some(Self::Enthusiastic),
            "VETERAN" => Some(Self::Veteran),
            "ELITE" => Some(Self::Elite),
            "HERO" => Some(Self::Hero),
            "BATTLEPLAN_BOMBARDMENT" => Some(Self::BattleplanBombardment),
            "BATTLEPLAN_HOLDTHELINE" => Some(Self::BattleplanHoldtheLine),
            "BATTLEPLAN_SEARCHANDDESTROY" => Some(Self::BattleplanSearchAndDestroy),
            "SUBLIMINAL" => Some(Self::Subliminal),
            "SOLO_HUMAN_EASY" => Some(Self::SoloHumanEasy),
            "SOLO_HUMAN_NORMAL" => Some(Self::SoloHumanNormal),
            "SOLO_HUMAN_HARD" => Some(Self::SoloHumanHard),
            "SOLO_AI_EASY" => Some(Self::SoloAiEasy),
            "SOLO_AI_NORMAL" => Some(Self::SoloAiNormal),
            "SOLO_AI_HARD" => Some(Self::SoloAiHard),
            "TARGET_FAERIE_FIRE" => Some(Self::TargetFaerieFire),
            "FANATICISM" => Some(Self::Fanaticism),
            "FRENZY_ONE" => Some(Self::FrenzyOne),
            "FRENZY_TWO" => Some(Self::FrenzyTwo),
            "FRENZY_THREE" => Some(Self::FrenzyThree),
            _ => None,
        }
    }
}

pub const WEAPON_BONUS_CONDITION_COUNT: usize = WEAPON_BONUS_NAMES.len();

/// Weapon bonus condition flags
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponBonusConditionFlags(u64);

impl WeaponBonusConditionFlags {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn empty() -> Self {
        Self::new()
    }
    /// Reconstitute the persisted condition mask captured by a projectile.
    pub fn from_bits(bits: u64) -> Self {
        Self(bits)
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

    pub fn union(&mut self, other: Self) {
        self.0 |= other.0;
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

impl WeaponBonusField {
    /// Parse a named GameData bonus entry.
    pub fn from_ini(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DAMAGE" => Some(Self::Damage),
            "RADIUS" => Some(Self::Radius),
            "RANGE" => Some(Self::Range),
            "RATE_OF_FIRE" => Some(Self::RateOfFire),
            "PRE_ATTACK" | "PREATTACK" => Some(Self::PreAttack),
            _ => None,
        }
    }
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

    /// Effective maximum reach after the range bonus and quarter-cell adjustment.
    pub fn attack_range(&self, range: f32) -> f32 {
        (range * self.get_field(WeaponBonusField::Range) - RANGE_UNDERSIZE).max(0.0)
    }

    /// Draw a delay from the firing session only when the authored bounds differ.
    pub fn delay_between_shots(
        &self,
        minimum: i32,
        maximum: i32,
        random: impl FnOnce(i32, i32) -> i32,
    ) -> i32 {
        self.reload_frames(draw_shot_delay(minimum, maximum, random))
    }

    /// A higher rate of fire reduces reload duration, rounded down to a frame.
    pub fn reload_frames(&self, duration: i32) -> i32 {
        (duration as f32 / self.get_field(WeaponBonusField::RateOfFire)).floor() as i32
    }

    /// The pre-attack multiplier truncates its result to a logic frame.
    pub fn pre_attack_frames(&self, delay: i32) -> i32 {
        (delay as f32 * self.get_field(WeaponBonusField::PreAttack)) as i32
    }

    pub fn damage(&self, damage: f32) -> f32 {
        damage * self.get_field(WeaponBonusField::Damage)
    }

    pub fn damage_radius(&self, radius: f32) -> f32 {
        radius * self.get_field(WeaponBonusField::Radius)
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
    bonuses: [Option<WeaponBonus>; WEAPON_BONUS_CONDITION_COUNT],
}

impl WeaponBonusSet {
    pub fn new() -> Self {
        Self {
            bonuses: std::array::from_fn(|_| None),
        }
    }

    pub fn set_bonus(&mut self, condition: WeaponBonusConditionType, bonus: WeaponBonus) {
        self.bonuses[condition as usize] = Some(bonus);
    }

    pub fn get_bonus(&self, condition: WeaponBonusConditionType) -> Option<&WeaponBonus> {
        self.bonuses[condition as usize].as_ref()
    }

    pub fn append_bonuses(&self, flags: WeaponBonusConditionFlags, bonus: &mut WeaponBonus) {
        for (index, weapon_bonus) in self.bonuses.iter().enumerate() {
            if flags.0 & (1 << index) != 0 {
                if let Some(weapon_bonus) = weapon_bonus {
                    bonus.append_bonuses(weapon_bonus);
                }
            }
        }
    }
}

impl Default for WeaponBonusSet {
    fn default() -> Self {
        Self::new()
    }
}
