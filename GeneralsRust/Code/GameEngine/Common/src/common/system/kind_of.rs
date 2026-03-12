//! KindOf classification system
//!
//! This module provides a bit flag system for classifying game objects by their
//! characteristics and capabilities. Objects can have multiple KindOf flags set
//! to indicate what they are and what they can do.

use bitflags::bitflags;
use std::fmt;

bitflags! {
    // KindOf flags for object classification
    // These flags determine groups of things that belong together and define
    // object capabilities and behaviors.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KindOfMask: u128 {
        const OBSTACLE = 1 << 0;                    // An obstacle to land-based pathfinders
        const SELECTABLE = 1 << 1;                  // Actually means MOUSE-INTERACTABLE
        const IMMOBILE = 1 << 2;                    // Fixed in location
        const CAN_ATTACK = 1 << 3;                  // Can attack
        const STICK_TO_TERRAIN_SLOPE = 1 << 4;      // Should be stuck at ground level, aligned to terrain slope
        const CAN_CAST_REFLECTIONS = 1 << 5;        // Can cast reflections in water
        const SHRUBBERY = 1 << 6;                   // Tree, bush, etc.
        const STRUCTURE = 1 << 7;                   // Structure of some sort (buildable or not)
        const INFANTRY = 1 << 8;                    // Unit like soldier etc
        const VEHICLE = 1 << 9;                     // Unit like tank, jeep, plane, helicopter, etc.
        const AIRCRAFT = 1 << 10;                   // Unit like plane, helicopter, etc., that is predominantly a flyer
        const HUGE_VEHICLE = 1 << 11;               // Unit that is technically a vehicle, but WAY larger than normal
        const DOZER = 1 << 12;                      // A dozer
        const HARVESTER = 1 << 13;                  // A harvester
        const COMMANDCENTER = 1 << 14;              // A command center
        const PRISON = 1 << 15;                     // A prison detention center kind of thing
        const COLLECTS_PRISON_BOUNTY = 1 << 16;     // When prisoners are delivered to these, the player gets money
        const POW_TRUCK = 1 << 17;                  // A POW truck can pick up and return prisoners
        const LINEBUILD = 1 << 18;                  // Wall-type thing that is built in a line
        const SALVAGER = 1 << 19;                   // Something that can create and use Salvage Crates
        const WEAPON_SALVAGER = 1 << 20;            // Subset of salvager that can get weapon upgrades from salvage
        const TRANSPORT = 1 << 21;                  // A true transport (has TransportContain)
        const BRIDGE = 1 << 22;                     // A Bridge (special structure)
        const LANDMARK_BRIDGE = 1 << 23;            // A landmark bridge (special bridge that isn't resizable)
        const BRIDGE_TOWER = 1 << 24;               // A bridge tower that we can target for bridge destruction
        const PROJECTILE = 1 << 25;                 // Instead of being a ground or air unit, this object is special
        const PRELOAD = 1 << 26;                    // All model data will be preloaded even if not on map
        const NO_GARRISON = 1 << 27;                // Unit may not garrison bldgs, even if infantry bit is set
        const WAVEGUIDE = 1 << 28;                  // Water wave object
        const WAVE_EFFECT = 1 << 29;                // Wave effect point
        const NO_COLLIDE = 1 << 30;                 // Never collide with or be collided with
        const REPAIR_PAD = 1 << 31;                 // Is a repair pad object that can repair other machines
        const HEAL_PAD = 1 << 32;                   // Is a heal pad object that can heal flesh and bone units
        const STEALTH_GARRISON = 1 << 33;           // Enemy teams can't tell that unit is in building
        const CASH_GENERATOR = 1 << 34;             // Used to check if the unit generates cash
        const DRAWABLE_ONLY = 1 << 35;              // Template is used only to create drawables (not Objects)
        const MP_COUNT_FOR_VICTORY = 1 << 36;       // If a player loses all his buildings that have this kindof in a multiplayer game, he loses
        const REBUILD_HOLE = 1 << 37;               // A GLA rebuild hole
        const SCORE = 1 << 38;                      // Object counts for Multiplayer scores, and short-game calculations
        const SCORE_CREATE = 1 << 39;               // Object only counts for multiplayer score for creation
        const SCORE_DESTROY = 1 << 40;              // Object only counts for multiplayer score for destruction
        const NO_HEAL_ICON = 1 << 41;               // Do not ever display healing icons on these objects
        const CAN_RAPPEL = 1 << 42;                 // Can rappel
        const PARACHUTABLE = 1 << 43;               // Parachutable object
        const CAN_SURRENDER = 1 << 44;              // Object that can surrender
        const CAN_BE_REPULSED = 1 << 45;            // Object that runs away from a repulsor object
        const MOB_NEXUS = 1 << 46;                  // Object that coordinates the members of a mob
        const IGNORED_IN_GUI = 1 << 47;             // Object that is the members of a mob
        const CRATE = 1 << 48;                      // A bonus crate
        const CAPTURABLE = 1 << 49;                 // Is "capturable" even if not an enemy
        const CLEARED_BY_BUILD = 1 << 50;           // Is auto-cleared from the map when built over via construction
        const SMALL_MISSILE = 1 << 51;              // Missile object: ONLY USED FOR ANTI-MISSILE TARGETTING PURPOSES!
        const ALWAYS_VISIBLE = 1 << 52;             // Is never obscured by fog of war or shroud
        const UNATTACKABLE = 1 << 53;               // You cannot target this thing, it probably doesn't really exist
        const MINE = 1 << 54;                       // A landmine
        const CLEANUP_HAZARD = 1 << 55;             // Radiation and bio-poison are samples of area conditions that can be cleaned up
        const PORTABLE_STRUCTURE = 1 << 56;         // Flag to identify building like subobjects an Overlord is allowed to Contain
        const ALWAYS_SELECTABLE = 1 << 57;          // Is never unselectable (even if effectively dead)
        const ATTACK_NEEDS_LINE_OF_SIGHT = 1 << 58; // Unit has to have clear line of sight (los) to attack
        const WALK_ON_TOP_OF_WALL = 1 << 59;        // Units can walk on top of a wall made of these kind of objects
        const DEFENSIVE_WALL = 1 << 60;             // Wall can't be driven through, even if crusher, so pathfinder must path around it
        const FS_POWER = 1 << 61;                   // Faction structure power building
        const FS_FACTORY = 1 << 62;                 // Faction structure factory building
        const FS_BASE_DEFENSE = 1 << 63;            // Faction structure base defense
        const FS_TECHNOLOGY = 1 << 64;              // Faction structure technology building
        const AIRCRAFT_PATH_AROUND = 1 << 65;       // Tall enough that aircraft need to path around this
        const LOW_OVERLAPPABLE = 1 << 66;           // When overlapped, things always overlap at a 'low' height
        const FORCEATTACKABLE = 1 << 67;            // Unit is always attackable via force-attack, even if not selectable
        const AUTO_RALLYPOINT = 1 << 68;            // When immobile-structure-object is selected, left clicking on ground will set new rally point
        const TECH_BUILDING = 1 << 69;              // Neutral tech building - Oil derrick, Hospital, Radio Station, Refinery
        const POWERED = 1 << 70;                    // This object gets the Underpowered disabled condition when its owning player has power consumption exceed supply
        const PRODUCED_AT_HELIPAD = 1 << 71;        // Hacky fix for comanche
        const DRONE = 1 << 72;                      // Object drone type -- used for filtering them out of battle plan bonuses
        const CAN_SEE_THROUGH_STRUCTURE = 1 << 73;  // Structure does not block line of sight
        const BALLISTIC_MISSILE = 1 << 74;          // Large ballistic missiles that are specifically large enough to be targeted by base defenses
        const CLICK_THROUGH = 1 << 75;              // Objects with this will never be picked by mouse interactions
        const SUPPLY_SOURCE_ON_PREVIEW = 1 << 76;   // Any thing that we can get "supplies" from that we want to show up on the map preview
        const PARACHUTE = 1 << 77;                  // It's a parachute
        const GARRISONABLE_UNTIL_DESTROYED = 1 << 78; // Object is capable of garrisoning troops until completely destroyed
        const BOAT = 1 << 79;                       // It's a boat!
        const IMMUNE_TO_CAPTURE = 1 << 80;          // Under no circumstances can this building ever be captured
        const HULK = 1 << 81;                       // Hulk types so we can do special things to them via scripts
        const SHOW_PORTRAIT_WHEN_CONTROLLED = 1 << 82; // Only shows portraits when controlled
        const SPAWNS_ARE_THE_WEAPONS = 1 << 83;     // Evaluate the spawn slaves as this object's weapons
        const CANNOT_BUILD_NEAR_SUPPLIES = 1 << 84; // You can't be built "too close" to anything that provides supplies
        const SUPPLY_SOURCE = 1 << 85;              // This object provides supplies
        const REVEAL_TO_ALL = 1 << 86;              // This object reveals shroud for all players
        const DISGUISER = 1 << 87;                  // This object has the ability to disguise
        const INERT = 1 << 88;                      // This object shouldn't be considered for any sort of interaction with any player
        const HERO = 1 << 89;                       // Any of the single-instance infantry, JarmenKell, BlackLotus, ColonelBurton
        const IGNORES_SELECT_ALL = 1 << 90;         // Too late to figure out intelligently if something should respond to a Select All command
        const DONT_AUTO_CRUSH_INFANTRY = 1 << 91;   // These units don't try to crush the infantry if ai
        const CLIFF_JUMPER = 1 << 92;               // Can't climb cliffs, but can jump off of them
        const FS_SUPPLY_DROPZONE = 1 << 93;         // A supply dropzone
        const FS_SUPERWEAPON = 1 << 94;             // A superweapon structure like a nuke silo, particle uplink cannon, scudstorm
        const FS_BLACK_MARKET = 1 << 95;            // Is this object a black market?
        const FS_SUPPLY_CENTER = 1 << 96;           // Is this object a supply center?
        const FS_STRATEGY_CENTER = 1 << 97;         // Is this object a strategy center?
        const MONEY_HACKER = 1 << 98;               // Money hacker
        const ARMOR_SALVAGER = 1 << 99;             // Armor salvager
        const REVEALS_ENEMY_PATHS = 1 << 100;       // Reveals enemy paths
        const BOOBY_TRAP = 1 << 101;                // Booby trap
        const FS_FAKE = 1 << 102;                   // Fake structure
        const FS_INTERNET_CENTER = 1 << 103;        // Internet center
        const BLAST_CRATER = 1 << 104;              // Blast crater
        const PROP = 1 << 105;                      // Prop
        const OPTIMIZED_TREE = 1 << 106;            // Optimized tree
        const FS_ADVANCED_TECH = 1 << 107;          // Advanced technology building
        const FS_BARRACKS = 1 << 108;               // Barracks
        const FS_WARFACTORY = 1 << 109;             // War factory
        const FS_AIRFIELD = 1 << 110;               // Airfield
        const AIRCRAFT_CARRIER = 1 << 111;          // Aircraft carrier
        const NO_SELECT = 1 << 112;                 // Cannot be selected
        const REJECT_UNMANNED = 1 << 113;           // Reject unmanned
        const CANNOT_RETALIATE = 1 << 114;          // Cannot retaliate
        const TECH_BASE_DEFENSE = 1 << 115;         // Tech base defense
        const EMP_HARDENED = 1 << 116;              // EMP hardened
        const DEMOTRAP = 1 << 117;                  // Demo trap
        const CONSERVATIVE_BUILDING = 1 << 118;     // Conservative building
        const IGNORE_DOCKING_BONES = 1 << 119;      // Ignore docking bones
    }
}

/// KindOf bit names matching the C++ s_bitNameList
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
    "PRISON",
    "COLLECTS_PRISON_BOUNTY",
    "POW_TRUCK",
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
    "CAN_SURRENDER",
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
}
