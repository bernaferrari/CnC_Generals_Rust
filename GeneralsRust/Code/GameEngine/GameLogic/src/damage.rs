//! Direct translation of the legacy damage system.
//!
//! Mirrors the original `Damage.h` / `Damage.cpp` definitions with full
//! behavior parity: types, enums, serialization, and helper functions.

use crate::common::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, ObjectID, ObjectStatusTypes, Real, Snapshot, ThingTemplate,
    xfer::Xfer,
};
pub use crate::common::{BodyDamageType, PlayerMaskType};
use bitflags::bitflags;
pub use generals_sim::damage::{DAMAGE_TYPE_COUNT, DAMAGE_TYPE_FLAG_NAMES, DamageType, DeathType};
use std::sync::Arc;

/// Original INI.cpp1780-1812 flag grammar, shared by typed module parsers.
pub mod flag_parsing;

/// Convenience aliases for body damage states used throughout legacy modules.
pub const BODY_PRISTINE: BodyDamageType = BodyDamageType::Pristine;
pub const BODY_DAMAGED: BodyDamageType = BodyDamageType::Damaged;
pub const BODY_REALLYDAMAGED: BodyDamageType = BodyDamageType::ReallyDamaged;
pub const BODY_RUBBLE: BodyDamageType = BodyDamageType::Rubble;

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
pub const HUGE_DAMAGE_AMOUNT: Real = 999_999.0;

/// Inputs that describe the damage being applied (`DamageInfoInput`).
#[derive(Debug, Clone)]
pub struct DamageInfoInput {
    pub source_id: ObjectID,
    pub source_template: Option<Arc<dyn ThingTemplate>>,
    pub source_player_mask: PlayerMaskType,
    pub damage_type: DamageType,
    pub damage_status_type: ObjectStatusTypes,
    pub damage_fx_override: DamageType,
    pub death_type: DeathType,
    pub amount: Real,
    pub kill: Bool,
    pub shock_wave_vector: Coord3D,
    pub shock_wave_amount: Real,
    pub shock_wave_radius: Real,
    pub shock_wave_taper_off: Real,
}

impl Default for DamageInfoInput {
    fn default() -> Self {
        Self {
            source_id: crate::common::INVALID_ID,
            source_template: None,
            source_player_mask: PlayerMaskType::none(),
            damage_type: DamageType::Explosion,
            damage_status_type: ObjectStatusTypes::None,
            damage_fx_override: DamageType::Unresistable,
            death_type: DeathType::Normal,
            amount: 0.0,
            kill: false,
            shock_wave_vector: Coord3D::new(0.0, 0.0, 0.0),
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
        }
    }
}

impl Snapshot for DamageInfoInput {
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // C++ DamageInfoInput::crc is empty. ActiveBody checksums go through xfer().
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 3;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        let mut source_id = self.source_id;
        let _ = xfer.xfer_unsigned_int(&mut source_id);
        self.source_id = source_id;

        let mut player_mask_bits = self.source_player_mask.bits() as u16;
        let _ = xfer.xfer_unsigned_short(&mut player_mask_bits);
        self.source_player_mask = PlayerMaskType::from_bits_truncate(player_mask_bits as u32);

        let mut damage_type = self.damage_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut damage_type);
        self.damage_type = DamageType::from_u32(damage_type);

        if version >= 2 {
            let mut damage_fx_override = self.damage_fx_override as u32;
            let _ = xfer.xfer_unsigned_int(&mut damage_fx_override);
            self.damage_fx_override = DamageType::from_u32(damage_fx_override);
        }

        let mut death_type = self.death_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut death_type);
        self.death_type = DeathType::from_u32(death_type);

        let mut amount = self.amount;
        let _ = xfer.xfer_real(&mut amount);
        self.amount = amount;

        // C++ tests currentVersion (the constant 3), not the loaded version.
        let mut kill = self.kill;
        let _ = xfer.xfer_bool(&mut kill);
        self.kill = kill;

        let mut status_type = self.damage_status_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut status_type);
        self.damage_status_type = ObjectStatusTypes::from_u32(status_type);

        let mut shock_vec = self.shock_wave_vector;
        let _ = xfer.xfer_coord3d(&mut shock_vec);
        self.shock_wave_vector = shock_vec;

        let mut shock_amount = self.shock_wave_amount;
        let _ = xfer.xfer_real(&mut shock_amount);
        self.shock_wave_amount = shock_amount;

        let mut shock_radius = self.shock_wave_radius;
        let _ = xfer.xfer_real(&mut shock_radius);
        self.shock_wave_radius = shock_radius;

        let mut shock_taper = self.shock_wave_taper_off;
        let _ = xfer.xfer_real(&mut shock_taper);
        self.shock_wave_taper_off = shock_taper;

        if version >= 3 {
            let mut thing_name = if let Some(template) = &self.source_template {
                template.get_name().clone()
            } else {
                AsciiString::TheEmptyString()
            };
            let _ = xfer.xfer_string(thing_name.as_mut_string_buffer());
            if xfer.is_loading() {
                self.source_template = crate::helpers::TheThingFactory::find_template(&thing_name);
            }
        }
    }

    fn load_post_process(&mut self) {}
}

/// Outputs populated once damage processing completes (`DamageInfoOutput`).
#[derive(Debug, Clone)]
pub struct DamageInfoOutput {
    pub actual_damage_dealt: Real,
    pub actual_damage_clipped: Real,
    pub no_effect: Bool,
    /// Whether this damage killed the target (set by Object::check_health_and_die)
    pub killed_target: Bool,
    /// Experience points awarded to attacker (set by Object::award_kill_experience)
    pub experience_awarded: Real,
}

impl Default for DamageInfoOutput {
    fn default() -> Self {
        Self {
            actual_damage_dealt: 0.0,
            actual_damage_clipped: 0.0,
            no_effect: false,
            killed_target: false,
            experience_awarded: 0.0,
        }
    }
}

impl Snapshot for DamageInfoOutput {
    fn crc(&self, _xfer: &mut dyn Xfer) {}

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        let mut dealt = self.actual_damage_dealt;
        let _ = xfer.xfer_real(&mut dealt);
        self.actual_damage_dealt = dealt;

        let mut clipped = self.actual_damage_clipped;
        let _ = xfer.xfer_real(&mut clipped);
        self.actual_damage_clipped = clipped;

        let mut no_effect = self.no_effect;
        let _ = xfer.xfer_bool(&mut no_effect);
        self.no_effect = no_effect;

        if xfer.is_loading() {
            self.killed_target = false;
            self.experience_awarded = 0.0;
        }
    }

    fn load_post_process(&mut self) {}
}

/// Combined snapshot describing both the requested damage and the resolved results.
#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    pub input: DamageInfoInput,
    pub output: DamageInfoOutput,
    /// Compatibility field - direct access to amount (same as input.amount)
    pub amount: Real,
    /// Compatibility field - direct access to damage_type (same as input.damage_type)
    pub damage_type: DamageType,
    /// Compatibility field - direct access to death_type (same as input.death_type)
    pub death_type: DeathType,
    /// Compatibility field - direct access to source_id (same as input.source_id)
    pub source_id: ObjectID,
}

impl Snapshot for DamageInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) {}

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        self.input.xfer(xfer);
        self.output.xfer(xfer);

        // Sync compatibility fields after xfer
        self.sync_from_input();
    }

    fn load_post_process(&mut self) {
        // Ensure compatibility fields are in sync after loading
        self.sync_from_input();
    }
}

impl DamageInfo {
    /// Create an empty damage packet (matching the old `DamageInfo::DamageInfo` ctor).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a damage packet with the common parameters used across the codebase.
    pub fn with_simple(
        amount: Real,
        source_id: ObjectID,
        damage_type: DamageType,
        death_type: DeathType,
    ) -> Self {
        let mut info = Self::default();
        info.input.amount = amount;
        info.input.source_id = source_id;
        info.input.damage_type = damage_type;
        info.input.death_type = death_type;
        // Sync compatibility fields
        info.amount = amount;
        info.source_id = source_id;
        info.damage_type = damage_type;
        info.death_type = death_type;
        info
    }

    /// Synchronize compatibility fields from input
    pub fn sync_from_input(&mut self) {
        self.amount = self.input.amount;
        self.damage_type = self.input.damage_type;
        self.death_type = self.input.death_type;
        self.source_id = self.input.source_id;
    }

    /// Synchronize input from compatibility fields
    pub fn sync_to_input(&mut self) {
        self.input.amount = self.amount;
        self.input.damage_type = self.damage_type;
        self.input.death_type = self.death_type;
        self.input.source_id = self.source_id;
    }
}

/// Initialise global damage type flags. Included for API parity with the C++ sources.
pub fn init_damage_type_flags() {
    // No-op in Rust: `DAMAGE_TYPE_FLAGS_ALL`/`NONE` are compile-time constants.
}
