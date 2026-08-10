// ModelConditionFlags and C++ alias constants
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

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
