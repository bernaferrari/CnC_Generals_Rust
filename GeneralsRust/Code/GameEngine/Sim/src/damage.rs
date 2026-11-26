//! Damage and death identities carried through combat, INI, and saves.

use std::str::FromStr;

/// Number of concrete damage types.
pub const DAMAGE_TYPE_COUNT: usize = DamageType::DamageNumTypes as usize;

/// Text labels for INI/debug usage (matches `DamageTypeFlags::s_bitNameList`).
pub const DAMAGE_TYPE_FLAG_NAMES: [&str; DAMAGE_TYPE_COUNT] = [
    "EXPLOSION",
    "CRUSH",
    "ARMOR_PIERCING",
    "SMALL_ARMS",
    "GATTLING",
    "RADIATION",
    "FLAME",
    "LASER",
    "SNIPER",
    "POISON",
    "HEALING",
    "UNRESISTABLE",
    "WATER",
    "DEPLOY",
    "SURRENDER",
    "HACK",
    "KILL_PILOT",
    "PENALTY",
    "FALLING",
    "MELEE",
    "DISARM",
    "HAZARD_CLEANUP",
    "PARTICLE_BEAM",
    "TOPPLING",
    "INFANTRY_MISSILE",
    "AURORA_BOMB",
    "LAND_MINE",
    "JET_MISSILES",
    "STEALTHJET_MISSILES",
    "MOLOTOV_COCKTAIL",
    "COMANCHE_VULCAN",
    "SUBDUAL_MISSILE",
    "SUBDUAL_VEHICLE",
    "SUBDUAL_BUILDING",
    "SUBDUAL_UNRESISTABLE",
    "MICROWAVE",
    "KILL_GARRISONED",
    "STATUS",
];

/// Damage type enumeration (`DamageType` in C++).
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DamageType {
    Explosion = 0,
    Crush = 1,
    ArmorPiercing = 2,
    SmallArms = 3,
    Gattling = 4,
    Radiation = 5,
    Flame = 6,
    Laser = 7,
    Sniper = 8,
    Poison = 9,
    Healing = 10,
    Unresistable = 11,
    Water = 12,
    Deploy = 13,
    Surrender = 14,
    Hack = 15,
    KillPilot = 16,
    Penalty = 17,
    Falling = 18,
    Melee = 19,
    Disarm = 20,
    HazardCleanup = 21,
    ParticleBeam = 22,
    Toppling = 23,
    InfantryMissile = 24,
    AuroraBomb = 25,
    LandMine = 26,
    JetMissiles = 27,
    StealthJetMissiles = 28,
    MolotovCocktail = 29,
    ComancheVulcan = 30,
    SubdualMissile = 31,
    SubdualVehicle = 32,
    SubdualBuilding = 33,
    SubdualUnresistable = 34,
    Microwave = 35,
    KillGarrisoned = 36,
    Status = 37,
    DamageNumTypes = 38,
}

impl Default for DamageType {
    fn default() -> Self {
        // C++ zero-initialized DamageType is DAMAGE_EXPLOSION (0), not UNRESISTABLE.
        DamageType::Explosion
    }
}

impl DamageType {
    /// Convert from a raw integer, clamping invalid values to `Explosion`.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => DamageType::Explosion,
            1 => DamageType::Crush,
            2 => DamageType::ArmorPiercing,
            3 => DamageType::SmallArms,
            4 => DamageType::Gattling,
            5 => DamageType::Radiation,
            6 => DamageType::Flame,
            7 => DamageType::Laser,
            8 => DamageType::Sniper,
            9 => DamageType::Poison,
            10 => DamageType::Healing,
            11 => DamageType::Unresistable,
            12 => DamageType::Water,
            13 => DamageType::Deploy,
            14 => DamageType::Surrender,
            15 => DamageType::Hack,
            16 => DamageType::KillPilot,
            17 => DamageType::Penalty,
            18 => DamageType::Falling,
            19 => DamageType::Melee,
            20 => DamageType::Disarm,
            21 => DamageType::HazardCleanup,
            22 => DamageType::ParticleBeam,
            23 => DamageType::Toppling,
            24 => DamageType::InfantryMissile,
            25 => DamageType::AuroraBomb,
            26 => DamageType::LandMine,
            27 => DamageType::JetMissiles,
            28 => DamageType::StealthJetMissiles,
            29 => DamageType::MolotovCocktail,
            30 => DamageType::ComancheVulcan,
            31 => DamageType::SubdualMissile,
            32 => DamageType::SubdualVehicle,
            33 => DamageType::SubdualBuilding,
            34 => DamageType::SubdualUnresistable,
            35 => DamageType::Microwave,
            36 => DamageType::KillGarrisoned,
            37 => DamageType::Status,
            _ => DamageType::Explosion,
        }
    }
}

impl FromStr for DamageType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "EXPLOSION" => Ok(DamageType::Explosion),
            "CRUSH" => Ok(DamageType::Crush),
            "ARMOR_PIERCING" => Ok(DamageType::ArmorPiercing),
            "SMALL_ARMS" => Ok(DamageType::SmallArms),
            "GATTLING" => Ok(DamageType::Gattling),
            "RADIATION" => Ok(DamageType::Radiation),
            "FLAME" => Ok(DamageType::Flame),
            "LASER" => Ok(DamageType::Laser),
            "SNIPER" => Ok(DamageType::Sniper),
            "POISON" => Ok(DamageType::Poison),
            "HEALING" => Ok(DamageType::Healing),
            "UNRESISTABLE" => Ok(DamageType::Unresistable),
            "WATER" => Ok(DamageType::Water),
            "DEPLOY" => Ok(DamageType::Deploy),
            "SURRENDER" => Ok(DamageType::Surrender),
            "HACK" => Ok(DamageType::Hack),
            "KILL_PILOT" => Ok(DamageType::KillPilot),
            "PENALTY" => Ok(DamageType::Penalty),
            "FALLING" => Ok(DamageType::Falling),
            "MELEE" => Ok(DamageType::Melee),
            "DISARM" => Ok(DamageType::Disarm),
            "HAZARD_CLEANUP" => Ok(DamageType::HazardCleanup),
            "PARTICLE_BEAM" => Ok(DamageType::ParticleBeam),
            "TOPPLING" => Ok(DamageType::Toppling),
            "INFANTRY_MISSILE" => Ok(DamageType::InfantryMissile),
            "AURORA_BOMB" => Ok(DamageType::AuroraBomb),
            "LAND_MINE" => Ok(DamageType::LandMine),
            "JET_MISSILES" => Ok(DamageType::JetMissiles),
            "STEALTHJET_MISSILES" => Ok(DamageType::StealthJetMissiles),
            "MOLOTOV_COCKTAIL" => Ok(DamageType::MolotovCocktail),
            "COMANCHE_VULCAN" => Ok(DamageType::ComancheVulcan),
            "SUBDUAL_MISSILE" => Ok(DamageType::SubdualMissile),
            "SUBDUAL_VEHICLE" => Ok(DamageType::SubdualVehicle),
            "SUBDUAL_BUILDING" => Ok(DamageType::SubdualBuilding),
            "SUBDUAL_UNRESISTABLE" => Ok(DamageType::SubdualUnresistable),
            "MICROWAVE" => Ok(DamageType::Microwave),
            "KILL_GARRISONED" => Ok(DamageType::KillGarrisoned),
            "STATUS" => Ok(DamageType::Status),
            _ => Err(()),
        }
    }
}

/// Death types (`DeathType` in the C++ sources).
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum DeathType {
    #[default]
    Normal = 0,
    None = 1,
    Crushed = 2,
    Burned = 3,
    Exploded = 4,
    Poisoned = 5,
    Toppled = 6,
    Flooded = 7,
    Suicided = 8,
    Lasered = 9,
    Detonated = 10,
    Splatted = 11,
    PoisonedBeta = 12,
    Extra2 = 13,
    Extra3 = 14,
    Extra4 = 15,
    Extra5 = 16,
    Extra6 = 17,
    Extra7 = 18,
    Extra8 = 19,
    PoisonedGamma = 20,
    DeathNumTypes = 21,
}

impl DeathType {
    /// Convert from raw integer, clamping invalid values to `Normal`.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => DeathType::Normal,
            1 => DeathType::None,
            2 => DeathType::Crushed,
            3 => DeathType::Burned,
            4 => DeathType::Exploded,
            5 => DeathType::Poisoned,
            6 => DeathType::Toppled,
            7 => DeathType::Flooded,
            8 => DeathType::Suicided,
            9 => DeathType::Lasered,
            10 => DeathType::Detonated,
            11 => DeathType::Splatted,
            12 => DeathType::PoisonedBeta,
            13 => DeathType::Extra2,
            14 => DeathType::Extra3,
            15 => DeathType::Extra4,
            16 => DeathType::Extra5,
            17 => DeathType::Extra6,
            18 => DeathType::Extra7,
            19 => DeathType::Extra8,
            20 => DeathType::PoisonedGamma,
            _ => DeathType::Normal,
        }
    }
}
