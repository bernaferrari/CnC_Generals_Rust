use super::{DAMAGE_TYPE_COUNT, DamageType, DeathType};
use bitflags::bitflags;

bitflags! {
    /// Bit-mask for damage type capabilities (`DamageTypeFlags`).
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DamageTypeFlags: u64 {
        const EXPLOSION             = 1 << DamageType::Explosion as u64;
        const CRUSH                 = 1 << DamageType::Crush as u64;
        const ARMOR_PIERCING        = 1 << DamageType::ArmorPiercing as u64;
        const SMALL_ARMS            = 1 << DamageType::SmallArms as u64;
        const GATTLING              = 1 << DamageType::Gattling as u64;
        const RADIATION             = 1 << DamageType::Radiation as u64;
        const FLAME                 = 1 << DamageType::Flame as u64;
        const LASER                 = 1 << DamageType::Laser as u64;
        const SNIPER                = 1 << DamageType::Sniper as u64;
        const POISON                = 1 << DamageType::Poison as u64;
        const HEALING               = 1 << DamageType::Healing as u64;
        const UNRESISTABLE          = 1 << DamageType::Unresistable as u64;
        const WATER                 = 1 << DamageType::Water as u64;
        const DEPLOY                = 1 << DamageType::Deploy as u64;
        const SURRENDER             = 1 << DamageType::Surrender as u64;
        const HACK                  = 1 << DamageType::Hack as u64;
        const KILL_PILOT            = 1 << DamageType::KillPilot as u64;
        const PENALTY               = 1 << DamageType::Penalty as u64;
        const FALLING               = 1 << DamageType::Falling as u64;
        const MELEE                 = 1 << DamageType::Melee as u64;
        const DISARM                = 1 << DamageType::Disarm as u64;
        const HAZARD_CLEANUP        = 1 << DamageType::HazardCleanup as u64;
        const PARTICLE_BEAM         = 1 << DamageType::ParticleBeam as u64;
        const TOPPLING              = 1 << DamageType::Toppling as u64;
        const INFANTRY_MISSILE      = 1 << DamageType::InfantryMissile as u64;
        const AURORA_BOMB           = 1 << DamageType::AuroraBomb as u64;
        const LAND_MINE             = 1 << DamageType::LandMine as u64;
        const JET_MISSILES          = 1 << DamageType::JetMissiles as u64;
        const STEALTHJET_MISSILES   = 1 << DamageType::StealthJetMissiles as u64;
        const MOLOTOV_COCKTAIL      = 1 << DamageType::MolotovCocktail as u64;
        const COMANCHE_VULCAN       = 1 << DamageType::ComancheVulcan as u64;
        const SUBDUAL_MISSILE       = 1 << DamageType::SubdualMissile as u64;
        const SUBDUAL_VEHICLE       = 1 << DamageType::SubdualVehicle as u64;
        const SUBDUAL_BUILDING      = 1 << DamageType::SubdualBuilding as u64;
        const SUBDUAL_UNRESISTABLE  = 1 << DamageType::SubdualUnresistable as u64;
        const MICROWAVE             = 1 << DamageType::Microwave as u64;
        const KILL_GARRISONED       = 1 << DamageType::KillGarrisoned as u64;
        const STATUS                = 1 << DamageType::Status as u64;
    }
}

impl DamageTypeFlags {
    /// Flag set that mirrors `SET_ALL_DAMAGE_TYPE_BITS`.
    pub const fn all_flags() -> Self {
        Self::from_bits_truncate((1u64 << DAMAGE_TYPE_COUNT) - 1)
    }

    /// Check if a specific damage type is set in this flag set.
    /// This is a convenience method that wraps the bitflags `contains` method.
    pub fn contains_damage_type(&self, damage_type: DamageType) -> bool {
        let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
        bitflags::Flags::contains(self, flag)
    }

    /// Test if a specific damage type is set (alias for `contains_damage_type`).
    pub fn test(&self, damage_type: DamageType) -> bool {
        self.contains_damage_type(damage_type)
    }

    /// Test if a specific damage type is set (alias for `contains_damage_type`).
    pub fn test_damage_type(&self, damage_type: DamageType) -> bool {
        self.contains_damage_type(damage_type)
    }

    /// Count the number of set damage type flags.
    pub fn count(&self) -> u32 {
        self.bits().count_ones()
    }

    /// Clear all damage type flags.
    pub fn clear_all(&mut self) {
        *self = Self::empty();
    }

    /// Set the specified damage type flag (C++-style convenience helper).
    pub fn set_damage_type(&mut self, damage_type: DamageType) {
        self.insert(DamageTypeFlags::from_bits_truncate(1 << damage_type as u64));
    }

    /// Clear the specified damage type flag (C++-style convenience helper).
    pub fn clear_damage_type(&mut self, damage_type: DamageType) {
        self.remove(DamageTypeFlags::from_bits_truncate(1 << damage_type as u64));
    }
}

/// Empty damage mask (`DAMAGE_TYPE_FLAGS_NONE`).
pub const DAMAGE_TYPE_FLAGS_NONE: DamageTypeFlags = DamageTypeFlags::empty();
/// Full damage mask (`DAMAGE_TYPE_FLAGS_ALL`).
pub const DAMAGE_TYPE_FLAGS_ALL: DamageTypeFlags = DamageTypeFlags::all_flags();

/// Check whether the specified damage flag is set.
pub fn get_damage_type_flag(flags: DamageTypeFlags, damage_type: DamageType) -> bool {
    bitflags::Flags::contains(
        &flags,
        DamageTypeFlags::from_bits_truncate(1 << damage_type as u64),
    )
}

/// Set the specified damage flag.
pub fn set_damage_type_flag(
    mut flags: DamageTypeFlags,
    damage_type: DamageType,
) -> DamageTypeFlags {
    flags.insert(DamageTypeFlags::from_bits_truncate(1 << damage_type as u64));
    flags
}

/// Clear the specified damage flag.
pub fn clear_damage_type_flag(
    mut flags: DamageTypeFlags,
    damage_type: DamageType,
) -> DamageTypeFlags {
    flags.remove(DamageTypeFlags::from_bits_truncate(1 << damage_type as u64));
    flags
}

/// Return `true` when this is one of the special subdual damage types.
pub fn is_subdual_damage(damage_type: DamageType) -> bool {
    matches!(
        damage_type,
        DamageType::SubdualMissile
            | DamageType::SubdualVehicle
            | DamageType::SubdualBuilding
            | DamageType::SubdualUnresistable
    )
}

/// Whether the damage type should reduce conventional hit points.
pub fn is_health_damaging_damage(damage_type: DamageType) -> bool {
    !matches!(
        damage_type,
        DamageType::Status
            | DamageType::SubdualMissile
            | DamageType::SubdualVehicle
            | DamageType::SubdualBuilding
            | DamageType::SubdualUnresistable
            | DamageType::KillPilot
            | DamageType::KillGarrisoned
    )
}

/// Reset a damage flag mask to contain every bit.
pub fn set_all_damage_type_bits(mask: &mut DamageTypeFlags) {
    *mask = DAMAGE_TYPE_FLAGS_ALL;
}

/// Death type mask (`DeathTypeFlags`).
pub type DeathTypeFlags = u32;

/// All death types enabled.
pub const DEATH_TYPE_FLAGS_ALL: DeathTypeFlags = 0xffff_ffff;
/// Empty death type flag set.
pub const DEATH_TYPE_FLAGS_NONE: DeathTypeFlags = 0x0000_0000;

/// Check a particular death flag.
pub fn get_death_type_flag(flags: DeathTypeFlags, death_type: DeathType) -> bool {
    // C++ `1UL << (dt - 1)`. dt 0 wraps; MSVC masks the count to 5 bits (bit 31).
    let bit = 1u32 << ((death_type as u32).wrapping_sub(1) & 31);
    (flags & bit) != 0
}

/// Set a particular death flag.
pub fn set_death_type_flag(flags: DeathTypeFlags, death_type: DeathType) -> DeathTypeFlags {
    let bit = 1u32 << ((death_type as u32).wrapping_sub(1) & 31);
    flags | bit
}

/// Clear a particular death flag.
pub fn clear_death_type_flag(flags: DeathTypeFlags, death_type: DeathType) -> DeathTypeFlags {
    let bit = 1u32 << ((death_type as u32).wrapping_sub(1) & 31);
    flags & !bit
}

/// Sentinel used by legacy scripts to request very large damage.
pub const HUGE_DAMAGE_AMOUNT: f32 = 999_999.0;
