////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: bit_flags.rs ///////////////////////////////////////////////////////////
//
// Used to set detail levels of various game systems.
//  Steven Johnson, Sept 2002
//
//
///////////////////////////////////////////////////////////////////////////////

use bit_vec::BitVec;

/// Model condition flags bit names
pub struct ModelConditionFlags;

impl ModelConditionFlags {
    pub const TOPPLED: usize = 0;
    pub const FRONTCRUSHED: usize = 1;
    pub const BACKCRUSHED: usize = 2;
    pub const DAMAGED: usize = 3;
    pub const REALLYDAMAGED: usize = 4;
    pub const RUBBLE: usize = 5;
    pub const SPECIAL_DAMAGED: usize = 6;
    pub const NIGHT: usize = 7;
    pub const SNOW: usize = 8;
    pub const PARACHUTING: usize = 9;
    pub const GARRISONED: usize = 10;
    pub const ENEMYNEAR: usize = 11;
    pub const WEAPONSET_VETERAN: usize = 12;
    pub const WEAPONSET_ELITE: usize = 13;
    pub const WEAPONSET_HERO: usize = 14;
    pub const WEAPONSET_CRATEUPGRADE_ONE: usize = 15;
    pub const WEAPONSET_CRATEUPGRADE_TWO: usize = 16;
    pub const WEAPONSET_PLAYER_UPGRADE: usize = 17;
    pub const DOOR_1_OPENING: usize = 18;
    pub const DOOR_1_CLOSING: usize = 19;
    pub const DOOR_1_WAITING_OPEN: usize = 20;
    pub const DOOR_1_WAITING_TO_CLOSE: usize = 21;
    pub const DOOR_2_OPENING: usize = 22;
    pub const DOOR_2_CLOSING: usize = 23;
    pub const DOOR_2_WAITING_OPEN: usize = 24;
    pub const DOOR_2_WAITING_TO_CLOSE: usize = 25;
    pub const DOOR_3_OPENING: usize = 26;
    pub const DOOR_3_CLOSING: usize = 27;
    pub const DOOR_3_WAITING_OPEN: usize = 28;
    pub const DOOR_3_WAITING_TO_CLOSE: usize = 29;
    pub const DOOR_4_OPENING: usize = 30;
    pub const DOOR_4_CLOSING: usize = 31;
    pub const DOOR_4_WAITING_OPEN: usize = 32;
    pub const DOOR_4_WAITING_TO_CLOSE: usize = 33;
    pub const ATTACKING: usize = 34;
    pub const PREATTACK_A: usize = 35;
    pub const FIRING_A: usize = 36;
    pub const BETWEEN_FIRING_SHOTS_A: usize = 37;
    pub const RELOADING_A: usize = 38;
    pub const PREATTACK_B: usize = 39;
    pub const FIRING_B: usize = 40;
    pub const BETWEEN_FIRING_SHOTS_B: usize = 41;
    pub const RELOADING_B: usize = 42;
    pub const PREATTACK_C: usize = 43;
    pub const FIRING_C: usize = 44;
    pub const BETWEEN_FIRING_SHOTS_C: usize = 45;
    pub const RELOADING_C: usize = 46;
    pub const TURRET_ROTATE: usize = 47;
    pub const POST_COLLAPSE: usize = 48;
    pub const MOVING: usize = 49;
    pub const DYING: usize = 50;
    pub const AWAITING_CONSTRUCTION: usize = 51;
    pub const PARTIALLY_CONSTRUCTED: usize = 52;
    pub const ACTIVELY_BEING_CONSTRUCTED: usize = 53;
    pub const PRONE: usize = 54;
    pub const FREEFALL: usize = 55;
    pub const ACTIVELY_CONSTRUCTING: usize = 56;
    pub const CONSTRUCTION_COMPLETE: usize = 57;
    pub const RADAR_EXTENDING: usize = 58;
    pub const RADAR_UPGRADED: usize = 59;
    pub const PANICKING: usize = 60; // yes, it's spelled with a "k". look it up.
    pub const AFLAME: usize = 61;
    pub const SMOLDERING: usize = 62;
    pub const BURNED: usize = 63;
    pub const DOCKING: usize = 64;
    pub const DOCKING_BEGINNING: usize = 65;
    pub const DOCKING_ACTIVE: usize = 66;
    pub const DOCKING_ENDING: usize = 67;
    pub const CARRYING: usize = 68;
    pub const FLOODED: usize = 69;
    pub const LOADED: usize = 70;
    pub const JETAFTERBURNER: usize = 71;
    pub const JETEXHAUST: usize = 72;
    pub const PACKING: usize = 73;
    pub const UNPACKING: usize = 74;
    pub const DEPLOYED: usize = 75;
    pub const OVER_WATER: usize = 76;
    pub const POWER_PLANT_UPGRADED: usize = 77;
    pub const CLIMBING: usize = 78;
    pub const SOLD: usize = 79;
    #[cfg(feature = "allow_surrender")]
    pub const SURRENDER: usize = 80;
    pub const RAPPELLING: usize = 81;
    pub const ARMED: usize = 82;
    pub const POWER_PLANT_UPGRADING: usize = 83;
    pub const SPECIAL_CHEERING: usize = 84;
    pub const CONTINUOUS_FIRE_SLOW: usize = 85;
    pub const CONTINUOUS_FIRE_MEAN: usize = 86;
    pub const CONTINUOUS_FIRE_FAST: usize = 87;
    pub const RAISING_FLAG: usize = 88;
    pub const CAPTURED: usize = 89;
    pub const EXPLODED_FLAILING: usize = 90;
    pub const EXPLODED_BOUNCING: usize = 91;
    pub const SPLATTED: usize = 92;
    pub const USING_WEAPON_A: usize = 93;
    pub const USING_WEAPON_B: usize = 94;
    pub const USING_WEAPON_C: usize = 95;
    pub const PREORDER: usize = 96;
    pub const CENTER_TO_LEFT: usize = 97;
    pub const LEFT_TO_CENTER: usize = 98;
    pub const CENTER_TO_RIGHT: usize = 99;
    pub const RIGHT_TO_CENTER: usize = 100;
    pub const RIDER1: usize = 101; // Kris: Added these for different combat-bike riders, but feel free to use these for anything.
    pub const RIDER2: usize = 102;
    pub const RIDER3: usize = 103;
    pub const RIDER4: usize = 104;
    pub const RIDER5: usize = 105;
    pub const RIDER6: usize = 106;
    pub const RIDER7: usize = 107;
    pub const RIDER8: usize = 108;
    pub const STUNNED_FLAILING: usize = 109; // Daniel Teh's idea, added by Lorenzen, 5/28/03
    pub const STUNNED: usize = 110;
    pub const SECOND_LIFE: usize = 111;
    pub const JAMMED: usize = 112;
    pub const ARMORSET_CRATEUPGRADE_ONE: usize = 113;
    pub const ARMORSET_CRATEUPGRADE_TWO: usize = 114;
    pub const USER_1: usize = 115;
    pub const USER_2: usize = 116;
    pub const DISGUISED: usize = 117;

    pub const BIT_NAMES: &'static [&'static str] = &[
        "TOPPLED",
        "FRONTCRUSHED",
        "BACKCRUSHED",
        "DAMAGED",
        "REALLYDAMAGED",
        "RUBBLE",
        "SPECIAL_DAMAGED",
        "NIGHT",
        "SNOW",
        "PARACHUTING",
        "GARRISONED",
        "ENEMYNEAR",
        "WEAPONSET_VETERAN",
        "WEAPONSET_ELITE",
        "WEAPONSET_HERO",
        "WEAPONSET_CRATEUPGRADE_ONE",
        "WEAPONSET_CRATEUPGRADE_TWO",
        "WEAPONSET_PLAYER_UPGRADE",
        "DOOR_1_OPENING",
        "DOOR_1_CLOSING",
        "DOOR_1_WAITING_OPEN",
        "DOOR_1_WAITING_TO_CLOSE",
        "DOOR_2_OPENING",
        "DOOR_2_CLOSING",
        "DOOR_2_WAITING_OPEN",
        "DOOR_2_WAITING_TO_CLOSE",
        "DOOR_3_OPENING",
        "DOOR_3_CLOSING",
        "DOOR_3_WAITING_OPEN",
        "DOOR_3_WAITING_TO_CLOSE",
        "DOOR_4_OPENING",
        "DOOR_4_CLOSING",
        "DOOR_4_WAITING_OPEN",
        "DOOR_4_WAITING_TO_CLOSE",
        "ATTACKING",
        "PREATTACK_A",
        "FIRING_A",
        "BETWEEN_FIRING_SHOTS_A",
        "RELOADING_A",
        "PREATTACK_B",
        "FIRING_B",
        "BETWEEN_FIRING_SHOTS_B",
        "RELOADING_B",
        "PREATTACK_C",
        "FIRING_C",
        "BETWEEN_FIRING_SHOTS_C",
        "RELOADING_C",
        "TURRET_ROTATE",
        "POST_COLLAPSE",
        "MOVING",
        "DYING",
        "AWAITING_CONSTRUCTION",
        "PARTIALLY_CONSTRUCTED",
        "ACTIVELY_BEING_CONSTRUCTED",
        "PRONE",
        "FREEFALL",
        "ACTIVELY_CONSTRUCTING",
        "CONSTRUCTION_COMPLETE",
        "RADAR_EXTENDING",
        "RADAR_UPGRADED",
        "PANICKING",
        "AFLAME",
        "SMOLDERING",
        "BURNED",
        "DOCKING",
        "DOCKING_BEGINNING",
        "DOCKING_ACTIVE",
        "DOCKING_ENDING",
        "CARRYING",
        "FLOODED",
        "LOADED",
        "JETAFTERBURNER",
        "JETEXHAUST",
        "PACKING",
        "UNPACKING",
        "DEPLOYED",
        "OVER_WATER",
        "POWER_PLANT_UPGRADED",
        "CLIMBING",
        "SOLD",
        #[cfg(feature = "allow_surrender")]
        "SURRENDER",
        "RAPPELLING",
        "ARMED",
        "POWER_PLANT_UPGRADING",
        "SPECIAL_CHEERING",
        "CONTINUOUS_FIRE_SLOW",
        "CONTINUOUS_FIRE_MEAN",
        "CONTINUOUS_FIRE_FAST",
        "RAISING_FLAG",
        "CAPTURED",
        "EXPLODED_FLAILING",
        "EXPLODED_BOUNCING",
        "SPLATTED",
        "USING_WEAPON_A",
        "USING_WEAPON_B",
        "USING_WEAPON_C",
        "PREORDER",
        "CENTER_TO_LEFT",
        "LEFT_TO_CENTER",
        "CENTER_TO_RIGHT",
        "RIGHT_TO_CENTER",
        "RIDER1",
        "RIDER2",
        "RIDER3",
        "RIDER4",
        "RIDER5",
        "RIDER6",
        "RIDER7",
        "RIDER8",
        "STUNNED_FLAILING",
        "STUNNED",
        "SECOND_LIFE",
        "JAMMED",
        "ARMORSET_CRATEUPGRADE_ONE",
        "ARMORSET_CRATEUPGRADE_TWO",
        "USER_1",
        "USER_2",
        "DISGUISED",
    ];
}

/// Armor set flags bit names
pub struct ArmorSetFlags;

impl ArmorSetFlags {
    pub const VETERAN: usize = 0;
    pub const ELITE: usize = 1;
    pub const HERO: usize = 2;
    pub const PLAYER_UPGRADE: usize = 3;
    pub const WEAK_VERSUS_BASEDEFENSES: usize = 4;
    pub const SECOND_LIFE: usize = 5;
    pub const CRATE_UPGRADE_ONE: usize = 6;
    pub const CRATE_UPGRADE_TWO: usize = 7;

    pub const BIT_NAMES: &'static [&'static str] = &[
        "VETERAN",
        "ELITE",
        "HERO",
        "PLAYER_UPGRADE",
        "WEAK_VERSUS_BASEDEFENSES",
        "SECOND_LIFE",
        "CRATE_UPGRADE_ONE",
        "CRATE_UPGRADE_TWO",
    ];
}

/// Weapon set flags bit names
pub struct WeaponSetFlags;

impl WeaponSetFlags {
    pub const VETERAN: usize = 0;
    pub const ELITE: usize = 1;
    pub const HERO: usize = 2;
    pub const PLAYER_UPGRADE: usize = 3;
    pub const CRATE_UPGRADE_ONE: usize = 4;
    pub const CRATE_UPGRADE_TWO: usize = 5;
    pub const VEHICLE_HIJACK: usize = 6;
    pub const CARBOMB: usize = 7;
    pub const MINE_CLEARING_DETAIL: usize = 8;
    pub const RIDER1: usize = 9;
    pub const RIDER2: usize = 10;
    pub const RIDER3: usize = 11;
    pub const RIDER4: usize = 12;
    pub const RIDER5: usize = 13;
    pub const RIDER6: usize = 14;
    pub const RIDER7: usize = 15;
    pub const RIDER8: usize = 16;

    pub const BIT_NAMES: &'static [&'static str] = &[
        "VETERAN",
        "ELITE",
        "HERO",
        "PLAYER_UPGRADE",
        "CRATE_UPGRADE_ONE",
        "CRATE_UPGRADE_TWO",
        "VEHICLE_HIJACK",
        "CARBOMB",
        "MINE_CLEARING_DETAIL",
        "RIDER1",
        "RIDER2",
        "RIDER3",
        "RIDER4",
        "RIDER5",
        "RIDER6",
        "RIDER7",
        "RIDER8",
    ];
}

/// A generic BitFlags structure that wraps a BitVec with name support
#[derive(Clone, Debug)]
pub struct BitFlags {
    bits: BitVec,
    bit_names: &'static [&'static str],
}

impl BitFlags {
    /// Create a new BitFlags with the specified bit names
    pub fn new(bit_names: &'static [&'static str]) -> Self {
        Self {
            bits: BitVec::from_elem(bit_names.len(), false),
            bit_names,
        }
    }

    /// Create a new BitFlags with specified bits set
    pub fn with_bits(bit_names: &'static [&'static str], indices: &[usize]) -> Self {
        let mut flags = Self::new(bit_names);
        for &idx in indices {
            if idx < flags.bits.len() {
                flags.bits.set(idx, true);
            }
        }
        flags
    }

    /// Set a bit at the given index
    pub fn set(&mut self, index: usize, value: bool) {
        if index < self.bits.len() {
            self.bits.set(index, value);
        }
    }

    /// Test if a bit is set at the given index
    pub fn test(&self, index: usize) -> bool {
        index < self.bits.len() && self.bits[index]
    }

    /// Test for any bits that are set in both self and other
    pub fn test_for_any(&self, other: &Self) -> bool {
        self.bits.iter().zip(other.bits.iter()).any(|(a, b)| a && b)
    }

    /// Test if all bits in other are also set in self
    pub fn test_for_all(&self, other: &Self) -> bool {
        // All argument bits must be set in our bits too in order to return true
        if !other.any() {
            panic!("BitFlags::test_for_all is always true if you ask about zero flags. Did you mean that?");
        }

        other
            .bits
            .iter()
            .enumerate()
            .all(|(i, bit)| !bit || self.test(i))
    }

    /// Test if none of the bits in other are set in self
    pub fn test_for_none(&self, other: &Self) -> bool {
        !self.test_for_any(other)
    }

    /// Get the size (number of possible bits)
    pub fn size(&self) -> usize {
        self.bits.len()
    }

    /// Count the number of set bits
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|&b| b).count()
    }

    /// Check if any bit is set
    pub fn any(&self) -> bool {
        self.bits.iter().any(|b| b)
    }

    /// Flip all bits
    pub fn flip(&mut self) {
        for i in 0..self.bits.len() {
            let val = self.bits[i];
            self.bits.set(i, !val);
        }
    }

    /// Clear all bits
    pub fn clear(&mut self) {
        for i in 0..self.bits.len() {
            self.bits.set(i, false);
        }
    }

    /// Count the number of bits that are set in both self and other
    pub fn count_intersection(&self, other: &Self) -> usize {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .filter(|(a, b)| *a && *b)
            .count()
    }

    /// Count the number of bits that are set in other but not in self
    pub fn count_inverse_intersection(&self, other: &Self) -> usize {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .filter(|(a, b)| !*a && *b)
            .count()
    }

    /// Check if there is any intersection with another BitFlags
    pub fn any_intersection_with(&self, other: &Self) -> bool {
        self.test_for_any(other)
    }

    /// Clear bits that are set in the clr BitFlags
    pub fn clear_bits(&mut self, clr: &Self) {
        for (i, bit) in clr.bits.iter().enumerate() {
            if bit && i < self.bits.len() {
                self.bits.set(i, false);
            }
        }
    }

    /// Set bits that are set in the set BitFlags
    pub fn set_bits(&mut self, set: &Self) {
        for (i, bit) in set.bits.iter().enumerate() {
            if bit && i < self.bits.len() {
                self.bits.set(i, true);
            }
        }
    }

    /// Clear and set bits in one operation
    pub fn clear_and_set(&mut self, clr: &Self, set: &Self) {
        self.clear_bits(clr);
        self.set_bits(set);
    }

    /// Test that specified bits are set and others are clear
    pub fn test_set_and_clear(&self, must_be_set: &Self, must_be_clear: &Self) -> bool {
        // Check that no bits in must_be_clear are set
        if self.test_for_any(must_be_clear) {
            return false;
        }

        // Check that all bits in must_be_set are set
        self.test_for_all(must_be_set)
    }

    /// Get bit names array
    pub fn get_bit_names(&self) -> &'static [&'static str] {
        self.bit_names
    }

    /// Get name from single bit index
    pub fn get_name_from_single_bit(&self, i: usize) -> Option<&'static str> {
        if i < self.bit_names.len() {
            Some(self.bit_names[i])
        } else {
            None
        }
    }

    /// Get single bit index from name
    pub fn get_single_bit_from_name(&self, token: &str) -> Option<usize> {
        self.bit_names
            .iter()
            .position(|&name| name.eq_ignore_ascii_case(token))
    }

    /// Get bit name if the bit is set
    pub fn get_bit_name_if_set(&self, i: usize) -> Option<&'static str> {
        if self.test(i) {
            self.get_name_from_single_bit(i)
        } else {
            None
        }
    }

    /// Set bit by name
    pub fn set_bit_by_name(&mut self, token: &str) -> bool {
        if let Some(i) = self.get_single_bit_from_name(token) {
            self.set(i, true);
            true
        } else {
            false
        }
    }

    /// Build description string of set bits
    pub fn build_description(&self) -> String {
        let mut result = String::new();

        for i in 0..self.size() {
            if let Some(bit_name) = self.get_bit_name_if_set(i) {
                if !result.is_empty() {
                    result.push_str(",\n");
                }
                result.push_str(bit_name);
            }
        }

        result
    }
}

impl PartialEq for BitFlags {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
    }
}

/// Type alias for model condition flags
pub type ModelConditionBitFlags = BitFlags;

/// Type alias for armor set flags  
pub type ArmorSetBitFlags = BitFlags;

/// Type alias for weapon set flags
pub type WeaponSetBitFlags = BitFlags;

/// Create model condition flags
pub fn create_model_condition_flags() -> ModelConditionBitFlags {
    BitFlags::new(ModelConditionFlags::BIT_NAMES)
}

/// Create armor set flags
pub fn create_armor_set_flags() -> ArmorSetBitFlags {
    BitFlags::new(ArmorSetFlags::BIT_NAMES)
}

/// Create weapon set flags
pub fn create_weapon_set_flags() -> WeaponSetBitFlags {
    BitFlags::new(WeaponSetFlags::BIT_NAMES)
}
