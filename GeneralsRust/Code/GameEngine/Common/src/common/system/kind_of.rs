//! KindOf classification system
//!
//! This module provides a bit flag system for classifying game objects by their
//! characteristics and capabilities. Objects can have multiple KindOf flags set
//! to indicate what they are and what they can do.
//!
//! Retail ZH does not define `ALLOW_SURRENDER`, so the live bit numbers match
//! `KindOf.h` / `KindOf.cpp` with PRISON, COLLECTS_PRISON_BOUNTY, POW_TRUCK,
//! and CAN_SURRENDER omitted. That yields `KINDOF_COUNT = 116`,
//! `ALWAYS_SELECTABLE = 53`, and `FORCEATTACKABLE = 63`.

use bitflags::bitflags;
use std::fmt;

/// Retail `KINDOF_COUNT` (`KindOf.h` last enumerator) with `ALLOW_SURRENDER` off.
pub const KINDOF_COUNT: usize = 116;

bitflags! {
    // KindOf flags for object classification.
    // Bit positions match C++ KindOfType when ALLOW_SURRENDER is undefined.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KindOfMask: u128 {
        const OBSTACLE = 1 << 0;
        const SELECTABLE = 1 << 1;
        const IMMOBILE = 1 << 2;
        const CAN_ATTACK = 1 << 3;
        const STICK_TO_TERRAIN_SLOPE = 1 << 4;
        const CAN_CAST_REFLECTIONS = 1 << 5;
        const SHRUBBERY = 1 << 6;
        const STRUCTURE = 1 << 7;
        const INFANTRY = 1 << 8;
        const VEHICLE = 1 << 9;
        const AIRCRAFT = 1 << 10;
        const HUGE_VEHICLE = 1 << 11;
        const DOZER = 1 << 12;
        const HARVESTER = 1 << 13;
        const COMMANDCENTER = 1 << 14;
        const LINEBUILD = 1 << 15;
        const SALVAGER = 1 << 16;
        const WEAPON_SALVAGER = 1 << 17;
        const TRANSPORT = 1 << 18;
        const BRIDGE = 1 << 19;
        const LANDMARK_BRIDGE = 1 << 20;
        const BRIDGE_TOWER = 1 << 21;
        const PROJECTILE = 1 << 22;
        const PRELOAD = 1 << 23;
        const NO_GARRISON = 1 << 24;
        const WAVEGUIDE = 1 << 25;
        const WAVE_EFFECT = 1 << 26;
        const NO_COLLIDE = 1 << 27;
        const REPAIR_PAD = 1 << 28;
        const HEAL_PAD = 1 << 29;
        const STEALTH_GARRISON = 1 << 30;
        const CASH_GENERATOR = 1 << 31;
        const DRAWABLE_ONLY = 1 << 32;
        const MP_COUNT_FOR_VICTORY = 1 << 33;
        const REBUILD_HOLE = 1 << 34;
        const SCORE = 1 << 35;
        const SCORE_CREATE = 1 << 36;
        const SCORE_DESTROY = 1 << 37;
        const NO_HEAL_ICON = 1 << 38;
        const CAN_RAPPEL = 1 << 39;
        const PARACHUTABLE = 1 << 40;
        const CAN_BE_REPULSED = 1 << 41;
        const MOB_NEXUS = 1 << 42;
        const IGNORED_IN_GUI = 1 << 43;
        const CRATE = 1 << 44;
        const CAPTURABLE = 1 << 45;
        const CLEARED_BY_BUILD = 1 << 46;
        const SMALL_MISSILE = 1 << 47;
        const ALWAYS_VISIBLE = 1 << 48;
        const UNATTACKABLE = 1 << 49;
        const MINE = 1 << 50;
        const CLEANUP_HAZARD = 1 << 51;
        const PORTABLE_STRUCTURE = 1 << 52;
        const ALWAYS_SELECTABLE = 1 << 53;
        const ATTACK_NEEDS_LINE_OF_SIGHT = 1 << 54;
        const WALK_ON_TOP_OF_WALL = 1 << 55;
        const DEFENSIVE_WALL = 1 << 56;
        const FS_POWER = 1 << 57;
        const FS_FACTORY = 1 << 58;
        const FS_BASE_DEFENSE = 1 << 59;
        const FS_TECHNOLOGY = 1 << 60;
        const AIRCRAFT_PATH_AROUND = 1 << 61;
        const LOW_OVERLAPPABLE = 1 << 62;
        const FORCEATTACKABLE = 1 << 63;
        const AUTO_RALLYPOINT = 1 << 64;
        const TECH_BUILDING = 1 << 65;
        const POWERED = 1 << 66;
        const PRODUCED_AT_HELIPAD = 1 << 67;
        const DRONE = 1 << 68;
        const CAN_SEE_THROUGH_STRUCTURE = 1 << 69;
        const BALLISTIC_MISSILE = 1 << 70;
        const CLICK_THROUGH = 1 << 71;
        const SUPPLY_SOURCE_ON_PREVIEW = 1 << 72;
        const PARACHUTE = 1 << 73;
        const GARRISONABLE_UNTIL_DESTROYED = 1 << 74;
        const BOAT = 1 << 75;
        const IMMUNE_TO_CAPTURE = 1 << 76;
        const HULK = 1 << 77;
        const SHOW_PORTRAIT_WHEN_CONTROLLED = 1 << 78;
        const SPAWNS_ARE_THE_WEAPONS = 1 << 79;
        const CANNOT_BUILD_NEAR_SUPPLIES = 1 << 80;
        const SUPPLY_SOURCE = 1 << 81;
        const REVEAL_TO_ALL = 1 << 82;
        const DISGUISER = 1 << 83;
        const INERT = 1 << 84;
        const HERO = 1 << 85;
        const IGNORES_SELECT_ALL = 1 << 86;
        const DONT_AUTO_CRUSH_INFANTRY = 1 << 87;
        const CLIFF_JUMPER = 1 << 88;
        const FS_SUPPLY_DROPZONE = 1 << 89;
        const FS_SUPERWEAPON = 1 << 90;
        const FS_BLACK_MARKET = 1 << 91;
        const FS_SUPPLY_CENTER = 1 << 92;
        const FS_STRATEGY_CENTER = 1 << 93;
        const MONEY_HACKER = 1 << 94;
        const ARMOR_SALVAGER = 1 << 95;
        const REVEALS_ENEMY_PATHS = 1 << 96;
        const BOOBY_TRAP = 1 << 97;
        const FS_FAKE = 1 << 98;
        const FS_INTERNET_CENTER = 1 << 99;
        const BLAST_CRATER = 1 << 100;
        const PROP = 1 << 101;
        const OPTIMIZED_TREE = 1 << 102;
        const FS_ADVANCED_TECH = 1 << 103;
        const FS_BARRACKS = 1 << 104;
        const FS_WARFACTORY = 1 << 105;
        const FS_AIRFIELD = 1 << 106;
        const AIRCRAFT_CARRIER = 1 << 107;
        const NO_SELECT = 1 << 108;
        const REJECT_UNMANNED = 1 << 109;
        const CANNOT_RETALIATE = 1 << 110;
        const TECH_BASE_DEFENSE = 1 << 111;
        const EMP_HARDENED = 1 << 112;
        const DEMOTRAP = 1 << 113;
        const CONSERVATIVE_BUILDING = 1 << 114;
        const IGNORE_DOCKING_BONES = 1 << 115;
    }
}

/// KindOf bit names matching C++ `KindOfMaskType::s_bitNameList` (`KindOf.cpp`)
/// with `ALLOW_SURRENDER` undefined. Index == bit position.
pub const KIND_OF_BIT_NAMES: &[&str] = &[
    "OBSTACLE",
    "SELECTABLE",
    "IMMOBILE",
    "CAN_ATTACK",
    "STICK_TO_TERRAIN_SLOPE",
    "CAN_CAST_REFLECTIONS",
    "SHRUBBERY",
    "STRUCTURE",
    "INFANTRY",
    "VEHICLE",
    "AIRCRAFT",
    "HUGE_VEHICLE",
    "DOZER",
    "HARVESTER",
    "COMMANDCENTER",
    "LINEBUILD",
    "SALVAGER",
    "WEAPON_SALVAGER",
    "TRANSPORT",
    "BRIDGE",
    "LANDMARK_BRIDGE",
    "BRIDGE_TOWER",
    "PROJECTILE",
    "PRELOAD",
    "NO_GARRISON",
    "WAVEGUIDE",
    "WAVE_EFFECT",
    "NO_COLLIDE",
    "REPAIR_PAD",
    "HEAL_PAD",
    "STEALTH_GARRISON",
    "CASH_GENERATOR",
    "DRAWABLE_ONLY",
    "MP_COUNT_FOR_VICTORY",
    "REBUILD_HOLE",
    "SCORE",
    "SCORE_CREATE",
    "SCORE_DESTROY",
    "NO_HEAL_ICON",
    "CAN_RAPPEL",
    "PARACHUTABLE",
    "CAN_BE_REPULSED",
    "MOB_NEXUS",
    "IGNORED_IN_GUI",
    "CRATE",
    "CAPTURABLE",
    "CLEARED_BY_BUILD",
    "SMALL_MISSILE",
    "ALWAYS_VISIBLE",
    "UNATTACKABLE",
    "MINE",
    "CLEANUP_HAZARD",
    "PORTABLE_STRUCTURE",
    "ALWAYS_SELECTABLE",
    "ATTACK_NEEDS_LINE_OF_SIGHT",
    "WALK_ON_TOP_OF_WALL",
    "DEFENSIVE_WALL",
    "FS_POWER",
    "FS_FACTORY",
    "FS_BASE_DEFENSE",
    "FS_TECHNOLOGY",
    "AIRCRAFT_PATH_AROUND",
    "LOW_OVERLAPPABLE",
    "FORCEATTACKABLE",
    "AUTO_RALLYPOINT",
    "TECH_BUILDING",
    "POWERED",
    "PRODUCED_AT_HELIPAD",
    "DRONE",
    "CAN_SEE_THROUGH_STRUCTURE",
    "BALLISTIC_MISSILE",
    "CLICK_THROUGH",
    "SUPPLY_SOURCE_ON_PREVIEW",
    "PARACHUTE",
    "GARRISONABLE_UNTIL_DESTROYED",
    "BOAT",
    "IMMUNE_TO_CAPTURE",
    "HULK",
    "SHOW_PORTRAIT_WHEN_CONTROLLED",
    "SPAWNS_ARE_THE_WEAPONS",
    "CANNOT_BUILD_NEAR_SUPPLIES",
    "SUPPLY_SOURCE",
    "REVEAL_TO_ALL",
    "DISGUISER",
    "INERT",
    "HERO",
    "IGNORES_SELECT_ALL",
    "DONT_AUTO_CRUSH_INFANTRY",
    "CLIFF_JUMPER",
    "FS_SUPPLY_DROPZONE",
    "FS_SUPERWEAPON",
    "FS_BLACK_MARKET",
    "FS_SUPPLY_CENTER",
    "FS_STRATEGY_CENTER",
    "MONEY_HACKER",
    "ARMOR_SALVAGER",
    "REVEALS_ENEMY_PATHS",
    "BOOBY_TRAP",
    "FS_FAKE",
    "FS_INTERNET_CENTER",
    "BLAST_CRATER",
    "PROP",
    "OPTIMIZED_TREE",
    "FS_ADVANCED_TECH",
    "FS_BARRACKS",
    "FS_WARFACTORY",
    "FS_AIRFIELD",
    "AIRCRAFT_CARRIER",
    "NO_SELECT",
    "REJECT_UNMANNED",
    "CANNOT_RETALIATE",
    "TECH_BASE_DEFENSE",
    "EMP_HARDENED",
    "DEMOTRAP",
    "CONSERVATIVE_BUILDING",
    "IGNORE_DOCKING_BONES",
];

/// Predefined KindOf mask constants
pub const KINDOFMASK_NONE: KindOfMask = KindOfMask::empty();

/// Faction structure mask (includes all FS_* flags)
pub const KINDOFMASK_FS: KindOfMask = KindOfMask::from_bits_truncate(
    KindOfMask::FS_FACTORY.bits()
        | KindOfMask::FS_BASE_DEFENSE.bits()
        | KindOfMask::FS_TECHNOLOGY.bits()
        | KindOfMask::FS_SUPPLY_DROPZONE.bits()
        | KindOfMask::FS_SUPERWEAPON.bits()
        | KindOfMask::FS_BLACK_MARKET.bits()
        | KindOfMask::FS_SUPPLY_CENTER.bits()
        | KindOfMask::FS_STRATEGY_CENTER.bits()
        | KindOfMask::FS_FAKE.bits()
        | KindOfMask::FS_INTERNET_CENTER.bits()
        | KindOfMask::FS_ADVANCED_TECH.bits()
        | KindOfMask::FS_BARRACKS.bits()
        | KindOfMask::FS_WARFACTORY.bits()
        | KindOfMask::FS_AIRFIELD.bits(),
);

impl KindOfMask {
    /// Parse a KindOf mask from a string name
    pub fn from_string(name: &str) -> Option<KindOfMask> {
        let upper_name = name.to_uppercase();

        // Find the bit position for this name
        if let Some(bit_index) = KIND_OF_BIT_NAMES
            .iter()
            .position(|&bit_name| bit_name == upper_name)
        {
            Some(KindOfMask::from_bits_truncate(1u128 << bit_index))
        } else {
            None
        }
    }

    /// Get a string representation of all set flags
    pub fn to_string_list(&self) -> Vec<String> {
        let mut flags = Vec::new();

        for (i, &name) in KIND_OF_BIT_NAMES.iter().enumerate() {
            if self.bits() & (1u128 << i) != 0 {
                flags.push(name.to_string());
            }
        }

        flags
    }

    /// Parse a KindOf token list the way C++ `BitFlags<NUMBITS>::parse` does.
    ///
    /// C++: `BitFlagsIO.h` lines 38-107. `NONE` clears and stops. `+NAME` / `-NAME`
    /// set or clear relative to `existing` (inherited / reskin-copied mask). A
    /// normal name list replaces `existing`. Mixing normal tokens with `+/-`
    /// or unknown names is an error (`INI_INVALID_NAME_LIST`).
    pub fn parse_ini(existing: KindOfMask, value: &str) -> Result<KindOfMask, String> {
        let mut mask = existing;
        let mut found_normal = false;
        let mut found_add_or_sub = false;

        for token in split_kindof_tokens(value) {
            if token == "NONE" {
                if found_normal || found_add_or_sub {
                    return Err(
                        "INI_INVALID_NAME_LIST: you may not mix normal and +- ops in bitstring lists"
                            .to_string(),
                    );
                }
                return Ok(KindOfMask::empty());
            }

            if let Some(name) = token.strip_prefix('+') {
                if found_normal {
                    return Err(
                        "INI_INVALID_NAME_LIST: you may not mix normal and +- ops in bitstring lists"
                            .to_string(),
                    );
                }
                let flag = KindOfMask::from_string(name).ok_or_else(|| {
                    format!("INI_INVALID_NAME_LIST: unknown KindOf token '{}'", name)
                })?;
                mask |= flag;
                found_add_or_sub = true;
            } else if let Some(name) = token.strip_prefix('-') {
                if found_normal {
                    return Err(
                        "INI_INVALID_NAME_LIST: you may not mix normal and +- ops in bitstring lists"
                            .to_string(),
                    );
                }
                let flag = KindOfMask::from_string(name).ok_or_else(|| {
                    format!("INI_INVALID_NAME_LIST: unknown KindOf token '{}'", name)
                })?;
                mask &= !flag;
                found_add_or_sub = true;
            } else {
                if found_add_or_sub {
                    return Err(
                        "INI_INVALID_NAME_LIST: you may not mix normal and +- ops in bitstring lists"
                            .to_string(),
                    );
                }
                if !found_normal {
                    mask = KindOfMask::empty();
                }
                let flag = KindOfMask::from_string(&token).ok_or_else(|| {
                    format!("INI_INVALID_NAME_LIST: unknown KindOf token '{}'", token)
                })?;
                mask |= flag;
                found_normal = true;
            }
        }

        Ok(mask)
    }

    /// C++ `TEST_KINDOFMASK_MULTI` (`KindOf.h`). Empty required/clear masks pass.
    #[inline]
    pub fn test_multi(self, must_be_set: Self, must_be_clear: Self) -> bool {
        self.contains(must_be_set) && !self.intersects(must_be_clear)
    }

    /// Check if this mask represents any kind of structure
    pub fn is_structure(&self) -> bool {
        self.contains(KindOfMask::STRUCTURE)
    }

    /// Check if this mask represents any kind of unit
    pub fn is_unit(&self) -> bool {
        self.intersects(KindOfMask::INFANTRY | KindOfMask::VEHICLE | KindOfMask::AIRCRAFT)
    }

    /// Check if this mask represents a faction structure
    pub fn is_faction_structure(&self) -> bool {
        self.intersects(KINDOFMASK_FS)
    }

    /// Check if this mask represents a military unit
    pub fn is_military(&self) -> bool {
        self.contains(KindOfMask::CAN_ATTACK) && self.is_unit()
    }

    /// Check if this mask represents a building that can be captured
    pub fn is_capturable_structure(&self) -> bool {
        self.contains(KindOfMask::CAPTURABLE) && self.contains(KindOfMask::STRUCTURE)
    }

    /// Check if this mask represents something that can be selected by the player
    pub fn is_player_selectable(&self) -> bool {
        self.contains(KindOfMask::SELECTABLE) && !self.contains(KindOfMask::NO_SELECT)
    }

    /// Check if this mask matches the given set and clear masks.
    /// Returns true if all bits in set_mask are set AND all bits in clear_mask are NOT set.
    /// Corresponds to C++ KindOfMaskType::matches()
    pub fn matches(&self, set_mask: KindOfMask, clear_mask: KindOfMask) -> bool {
        self.contains(set_mask) && !self.intersects(clear_mask)
    }

    /// Check if this mask contains all bits from another mask.
    /// Returns true if every bit set in `other` is also set in `self`.
    /// Corresponds to C++ containsAll behavior.
    pub fn contains_all(&self, other: KindOfMask) -> bool {
        self.contains(other)
    }
}

fn split_kindof_tokens(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(|c: char| c == '|' || c == ',' || c.is_whitespace())
        .filter_map(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_uppercase())
            }
        })
}

impl fmt::Display for KindOfMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flags = self.to_string_list();
        if flags.is_empty() {
            write!(f, "NONE")
        } else {
            write!(f, "{}", flags.join(" | "))
        }
    }
}

/// Maps a C++ `KindOfType` sequential integer to the corresponding `KindOfMask` bitflag.
///
/// In C++, `KindOfMaskType` is `BitFlags<KINDOF_COUNT>` where each `KindOfType` enum value
/// doubles as a bit position (e.g. `KINDOF_OBSTACLE = 0` → bit 0, `KINDOF_INFANTRY = 8` → bit 8).
/// The Rust `KindOfMask` bitflags use the same bit positions as the C++ enum values,
/// so this is a direct `1 << kind_type` conversion.
///
/// Retail ZH leaves `ALLOW_SURRENDER` undefined, so PRISON / COLLECTS_PRISON_BOUNTY /
/// POW_TRUCK / CAN_SURRENDER are omitted and later bits sit at the C++ header values
/// (`ALWAYS_SELECTABLE = 53`, `FORCEATTACKABLE = 63`, `KINDOF_COUNT = 116`).
///
/// Returns `None` for `KINDOF_INVALID` (-1), `KINDOF_COUNT`, or any out-of-range value.
pub fn kind_of_type_to_mask(kind_type: i32) -> Option<KindOfMask> {
    if kind_type < 0 || kind_type as usize >= KIND_OF_BIT_NAMES.len() {
        return None;
    }
    let mask = KindOfMask::from_bits_truncate(1u128 << kind_type);
    // Verify the bit is actually defined (not just within range of u128).
    if mask.is_empty() {
        return None;
    }
    Some(mask)
}

/// Reverse mapping: given a `KindOfMask` containing exactly one bit set, return the
/// corresponding C++ `KindOfType` sequential integer. Returns `None` if zero or
/// multiple bits are set.
pub fn mask_to_kind_of_type(mask: KindOfMask) -> Option<i32> {
    let bits = mask.bits();
    if bits == 0 || (bits & (bits - 1)) != 0 {
        return None; // Zero or multiple bits set
    }
    // Find the trailing zero count = bit position
    let pos = bits.trailing_zeros() as i32;
    if pos as usize >= KIND_OF_BIT_NAMES.len() {
        return None;
    }
    Some(pos)
}

/// Look up the retail `KindOf.h` bit index for an INI / BitFlags name.
///
/// Names match `KIND_OF_BIT_NAMES` (`KindOf.cpp` with `ALLOW_SURRENDER` off).
/// Returns `None` for invented tokens (UNIT, WEAPON, BARRIER, …) and surrender-only
/// names (PRISON, POW_TRUCK, CAN_SURRENDER).
pub fn kind_of_bit_from_name(name: &str) -> Option<u32> {
    let upper = name.trim().to_ascii_uppercase();
    KIND_OF_BIT_NAMES
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(&upper))
        .map(|idx| idx as u32)
}

/// Initialize KindOf masks (corresponds to initKindOfMasks() in C++)
pub fn init_kind_of_masks() {
    // This function was used to initialize global masks in C++
    // In Rust, we use const definitions instead, but this function
    // is provided for API compatibility
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_flags() {
        let mut mask = KindOfMask::empty();
        assert!(!mask.contains(KindOfMask::STRUCTURE));

        mask |= KindOfMask::STRUCTURE;
        assert!(mask.contains(KindOfMask::STRUCTURE));
        assert!(mask.is_structure());

        mask |= KindOfMask::INFANTRY;
        assert!(mask.contains(KindOfMask::INFANTRY));
        assert!(mask.is_unit());
    }

    #[test]
    fn test_from_name() {
        assert_eq!(
            KindOfMask::from_string("STRUCTURE"),
            Some(KindOfMask::STRUCTURE)
        );
        assert_eq!(
            KindOfMask::from_string("structure"),
            Some(KindOfMask::STRUCTURE)
        ); // Case insensitive
        assert_eq!(KindOfMask::from_string("INVALID_FLAG"), None);
        assert_eq!(KindOfMask::from_string("PRISON"), None);
        assert_eq!(KindOfMask::from_string("CAN_SURRENDER"), None);
    }

    #[test]
    fn test_to_string_list() {
        let mask = KindOfMask::STRUCTURE | KindOfMask::INFANTRY;
        let flags = mask.to_string_list();

        assert!(flags.contains(&"STRUCTURE".to_string()));
        assert!(flags.contains(&"INFANTRY".to_string()));
        assert_eq!(flags.len(), 2);
    }

    #[test]
    fn test_is_methods() {
        let structure = KindOfMask::STRUCTURE;
        assert!(structure.is_structure());
        assert!(!structure.is_unit());

        let infantry = KindOfMask::INFANTRY;
        assert!(infantry.is_unit());
        assert!(!infantry.is_structure());

        let military_unit = KindOfMask::INFANTRY | KindOfMask::CAN_ATTACK;
        assert!(military_unit.is_military());

        let faction_structure = KindOfMask::FS_FACTORY;
        assert!(faction_structure.is_faction_structure());

        let capturable = KindOfMask::STRUCTURE | KindOfMask::CAPTURABLE;
        assert!(capturable.is_capturable_structure());

        let selectable = KindOfMask::SELECTABLE;
        assert!(selectable.is_player_selectable());

        let not_selectable = KindOfMask::SELECTABLE | KindOfMask::NO_SELECT;
        assert!(!not_selectable.is_player_selectable());
    }

    #[test]
    fn test_predefined_masks() {
        assert!(KINDOFMASK_NONE.is_empty());
        assert!(!KINDOFMASK_FS.is_empty());
        assert!(KINDOFMASK_FS.contains(KindOfMask::FS_FACTORY));
        assert!(KINDOFMASK_FS.contains(KindOfMask::FS_BASE_DEFENSE));
    }

    #[test]
    fn test_display_format() {
        let empty_mask = KindOfMask::empty();
        assert_eq!(format!("{}", empty_mask), "NONE");

        let structure_mask = KindOfMask::STRUCTURE;
        assert_eq!(format!("{}", structure_mask), "STRUCTURE");

        let combined_mask = KindOfMask::STRUCTURE | KindOfMask::SELECTABLE;
        let display_str = format!("{}", combined_mask);
        assert!(display_str.contains("STRUCTURE"));
        assert!(display_str.contains("SELECTABLE"));
        assert!(display_str.contains(" | "));
    }

    #[test]
    fn test_bit_names_consistency() {
        // Test that bit names array matches our flag definitions
        assert_eq!(KIND_OF_BIT_NAMES[0], "OBSTACLE");
        assert_eq!(KIND_OF_BIT_NAMES[1], "SELECTABLE");
        assert_eq!(KIND_OF_BIT_NAMES[7], "STRUCTURE");
        assert_eq!(KIND_OF_BIT_NAMES[8], "INFANTRY");
        assert_eq!(KIND_OF_BIT_NAMES[15], "LINEBUILD");

        // Test that we can parse all our bit names
        for &name in KIND_OF_BIT_NAMES {
            if !name.is_empty() {
                assert!(
                    KindOfMask::from_string(name).is_some(),
                    "Failed to parse: {}",
                    name
                );
            }
        }
    }

    // C++ KindOf.h:38-73 / KindOf.h:150 — retail ALLOW_SURRENDER is off.
    #[test]
    fn retail_kindof_bit_numbers_match_kind_of_h() {
        assert_eq!(KIND_OF_BIT_NAMES.len(), KINDOF_COUNT);
        assert_eq!(KINDOF_COUNT, 116);
        assert_eq!(KindOfMask::ALWAYS_SELECTABLE.bits().trailing_zeros(), 53);
        assert_eq!(KindOfMask::FORCEATTACKABLE.bits().trailing_zeros(), 63);
        assert_eq!(KindOfMask::LINEBUILD.bits().trailing_zeros(), 15);
        assert_eq!(KIND_OF_BIT_NAMES[53], "ALWAYS_SELECTABLE");
        assert_eq!(KIND_OF_BIT_NAMES[63], "FORCEATTACKABLE");
        assert_eq!(
            kind_of_type_to_mask(53),
            Some(KindOfMask::ALWAYS_SELECTABLE)
        );
        assert_eq!(kind_of_type_to_mask(63), Some(KindOfMask::FORCEATTACKABLE));
        assert_eq!(kind_of_type_to_mask(KINDOF_COUNT as i32), None);
    }

    // C++ BitFlagsIO.h:38-107 — +NAME is incremental; unknown names throw.
    #[test]
    fn parse_ini_plus_hero_is_incremental() {
        let inherited = KindOfMask::INFANTRY | KindOfMask::SELECTABLE;
        let parsed = KindOfMask::parse_ini(inherited, "+HERO").expect("+HERO should parse");
        assert!(parsed.contains(KindOfMask::INFANTRY));
        assert!(parsed.contains(KindOfMask::SELECTABLE));
        assert!(parsed.contains(KindOfMask::HERO));
    }

    #[test]
    fn parse_ini_unknown_name_errors() {
        let err = KindOfMask::parse_ini(KindOfMask::empty(), "NOT_A_REAL_KIND")
            .expect_err("unknown token must error");
        assert!(err.contains("NOT_A_REAL_KIND"), "{err}");
        assert!(err.contains("INI_INVALID_NAME_LIST"), "{err}");
    }

    #[test]
    fn parse_ini_minus_and_none() {
        let inherited = KindOfMask::INFANTRY | KindOfMask::HERO;
        let cleared = KindOfMask::parse_ini(inherited, "-HERO").expect("-HERO should parse");
        assert!(cleared.contains(KindOfMask::INFANTRY));
        assert!(!cleared.contains(KindOfMask::HERO));

        let none = KindOfMask::parse_ini(inherited, "NONE").expect("NONE should parse");
        assert!(none.is_empty());
    }
}
