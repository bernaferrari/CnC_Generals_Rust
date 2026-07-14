//! Wave 82: residual enum / bit-name table honesty packs.
//!
//! Freezes C++ ordered name tables used by INI parsers and save/load:
//! - DamageTypeFlags::s_bitNameList (Damage.cpp) — DAMAGE_NUM_TYPES **38**
//! - TheDeathNames (Damage.h DEFINE_DEATH_NAMES) — DEATH_NUM_TYPES **21**
//! - ModelConditionFlags::s_bitNameList (BitFlags.cpp) — MODELCONDITION_COUNT **117**
//!   (ALLOW_SURRENDER off — no SURRENDER bit between SOLD and RAPPELLING)
//! - TheWeaponBonusNames (Weapon.h, ALLOW_DEMORALIZE off) — COUNT **27**
//! - ObjectStatusMaskType::s_bitNameList (ObjectStatusTypes.cpp) — COUNT **45**
//!
//! Fail-closed:
//! - Not full armor/weapon combat application of every discriminant
//! - Not full W3D MODELCONDITION anim draw matrix
//! - Not full ObjectStatus Xfer rebind / StatusBitsUpgrade matrix
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// DamageType residual table (Damage.cpp s_bitNameList)
// ---------------------------------------------------------------------------

/// C++ `DAMAGE_NUM_TYPES` residual (Damage.h).
pub const DAMAGE_NUM_TYPES: u32 = 38;

/// Ordered C++ `DamageTypeFlags::s_bitNameList` residual (excluding trailing NULL).
pub const DAMAGE_TYPE_BIT_NAME_LIST: &[&str] = &[
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

/// Lookup DamageType bit-name index residual.
pub fn damage_type_bit_name_index(name: &str) -> Option<usize> {
    DAMAGE_TYPE_BIT_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 82 honesty: DamageType residual enum table pack.
pub fn honesty_damage_type_enum_table_wave82() -> bool {
    DAMAGE_NUM_TYPES == 38
        && DAMAGE_TYPE_BIT_NAME_LIST.len() == 38
        && DAMAGE_TYPE_BIT_NAME_LIST[0] == "EXPLOSION"
        && DAMAGE_TYPE_BIT_NAME_LIST[3] == "SMALL_ARMS"
        && DAMAGE_TYPE_BIT_NAME_LIST[6] == "FLAME"
        && DAMAGE_TYPE_BIT_NAME_LIST[9] == "POISON"
        && DAMAGE_TYPE_BIT_NAME_LIST[11] == "UNRESISTABLE"
        && DAMAGE_TYPE_BIT_NAME_LIST[22] == "PARTICLE_BEAM"
        && DAMAGE_TYPE_BIT_NAME_LIST[31] == "SUBDUAL_MISSILE"
        && DAMAGE_TYPE_BIT_NAME_LIST[35] == "MICROWAVE"
        && DAMAGE_TYPE_BIT_NAME_LIST[37] == "STATUS"
        && damage_type_bit_name_index("STATUS") == Some(37)
        && damage_type_bit_name_index("EXPLOSION") == Some(0)
        && damage_type_bit_name_index("PARTICLE_BEAM") == Some(22)
        // Subdual residual cluster contiguous.
        && damage_type_bit_name_index("SUBDUAL_MISSILE") == Some(31)
        && damage_type_bit_name_index("SUBDUAL_VEHICLE") == Some(32)
        && damage_type_bit_name_index("SUBDUAL_BUILDING") == Some(33)
        && damage_type_bit_name_index("SUBDUAL_UNRESISTABLE") == Some(34)
        // Unique names.
        && {
            let mut names: Vec<&str> = DAMAGE_TYPE_BIT_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// DeathType residual table (Damage.h TheDeathNames)
// ---------------------------------------------------------------------------

/// C++ `DEATH_NUM_TYPES` residual (Damage.h).
pub const DEATH_NUM_TYPES: u32 = 21;

/// Ordered C++ `TheDeathNames` residual (DEFINE_DEATH_NAMES), excluding NULL.
pub const DEATH_TYPE_NAME_LIST: &[&str] = &[
    "NORMAL",
    "NONE",
    "CRUSHED",
    "BURNED",
    "EXPLODED",
    "POISONED",
    "TOPPLED",
    "FLOODED",
    "SUICIDED",
    "LASERED",
    "DETONATED",
    "SPLATTED",
    "POISONED_BETA",
    "EXTRA_2",
    "EXTRA_3",
    "EXTRA_4",
    "EXTRA_5",
    "EXTRA_6",
    "EXTRA_7",
    "EXTRA_8",
    "POISONED_GAMMA",
];

/// Lookup DeathType name index residual.
pub fn death_type_name_index(name: &str) -> Option<usize> {
    DEATH_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 82 honesty: DeathType residual enum table pack.
pub fn honesty_death_type_enum_table_wave82() -> bool {
    DEATH_NUM_TYPES == 21
        && DEATH_TYPE_NAME_LIST.len() == 21
        && DEATH_TYPE_NAME_LIST[0] == "NORMAL"
        && DEATH_TYPE_NAME_LIST[1] == "NONE"
        && DEATH_TYPE_NAME_LIST[3] == "BURNED"
        && DEATH_TYPE_NAME_LIST[4] == "EXPLODED"
        && DEATH_TYPE_NAME_LIST[5] == "POISONED"
        && DEATH_TYPE_NAME_LIST[8] == "SUICIDED"
        && DEATH_TYPE_NAME_LIST[10] == "DETONATED"
        && DEATH_TYPE_NAME_LIST[11] == "SPLATTED"
        && DEATH_TYPE_NAME_LIST[12] == "POISONED_BETA"
        && DEATH_TYPE_NAME_LIST[20] == "POISONED_GAMMA"
        && death_type_name_index("NORMAL") == Some(0)
        && death_type_name_index("EXPLODED") == Some(4)
        && death_type_name_index("POISONED_GAMMA") == Some(20)
        // Death names deliberately differ from damage names residual sample.
        && DEATH_TYPE_NAME_LIST[3] != "FLAME"
        && DEATH_TYPE_NAME_LIST[4] != "EXPLOSION"
        && {
            let mut names: Vec<&str> = DEATH_TYPE_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// ModelCondition residual table (BitFlags.cpp; ALLOW_SURRENDER off)
// ---------------------------------------------------------------------------

/// C++ `MODELCONDITION_COUNT` residual without ALLOW_SURRENDER (ModelState.h).
pub const MODELCONDITION_COUNT: u32 = 117;

/// Ordered C++ `ModelConditionFlags::s_bitNameList` residual (ALLOW_SURRENDER off).
///
/// SURRENDER is not present between SOLD and RAPPELLING in retail ZH builds.
pub const MODEL_CONDITION_BIT_NAME_LIST: &[&str] = &[
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
    // ALLOW_SURRENDER off — no SURRENDER entry here
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

/// C++ MODELCONDITION_CONTINUOUS_FIRE_* ordinal residual (ALLOW_SURRENDER off).
pub const MODELCONDITION_CONTINUOUS_FIRE_SLOW: u32 = 84;
/// C++ MODELCONDITION_CONTINUOUS_FIRE_MEAN residual.
pub const MODELCONDITION_CONTINUOUS_FIRE_MEAN: u32 = 85;
/// C++ MODELCONDITION_CONTINUOUS_FIRE_FAST residual.
pub const MODELCONDITION_CONTINUOUS_FIRE_FAST: u32 = 86;

/// Lookup ModelCondition bit-name index residual.
pub fn model_condition_bit_name_index(name: &str) -> Option<usize> {
    MODEL_CONDITION_BIT_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 82 honesty: ModelCondition residual flags table pack (incl. CONTINUOUS_FIRE_*).
pub fn honesty_model_condition_enum_table_wave82() -> bool {
    MODELCONDITION_COUNT == 117
        && MODEL_CONDITION_BIT_NAME_LIST.len() == 117
        && MODEL_CONDITION_BIT_NAME_LIST[0] == "TOPPLED"
        && MODEL_CONDITION_BIT_NAME_LIST[3] == "DAMAGED"
        && MODEL_CONDITION_BIT_NAME_LIST[4] == "REALLYDAMAGED"
        && MODEL_CONDITION_BIT_NAME_LIST[35] == "PREATTACK_A"
        && MODEL_CONDITION_BIT_NAME_LIST[36] == "FIRING_A"
        && MODEL_CONDITION_BIT_NAME_LIST[49] == "MOVING"
        && MODEL_CONDITION_BIT_NAME_LIST[79] == "SOLD"
        && MODEL_CONDITION_BIT_NAME_LIST[80] == "RAPPELLING"
        // No SURRENDER residual in ZH (ALLOW_SURRENDER off).
        && !MODEL_CONDITION_BIT_NAME_LIST
            .iter()
            .any(|&n| n == "SURRENDER")
        && MODEL_CONDITION_BIT_NAME_LIST[84] == "CONTINUOUS_FIRE_SLOW"
        && MODEL_CONDITION_BIT_NAME_LIST[85] == "CONTINUOUS_FIRE_MEAN"
        && MODEL_CONDITION_BIT_NAME_LIST[86] == "CONTINUOUS_FIRE_FAST"
        && MODELCONDITION_CONTINUOUS_FIRE_SLOW == 84
        && MODELCONDITION_CONTINUOUS_FIRE_MEAN == 85
        && MODELCONDITION_CONTINUOUS_FIRE_FAST == 86
        && model_condition_bit_name_index("CONTINUOUS_FIRE_SLOW") == Some(84)
        && model_condition_bit_name_index("CONTINUOUS_FIRE_MEAN") == Some(85)
        && model_condition_bit_name_index("CONTINUOUS_FIRE_FAST") == Some(86)
        && MODEL_CONDITION_BIT_NAME_LIST[116] == "DISGUISED"
        && model_condition_bit_name_index("DISGUISED") == Some(116)
        // Contiguous CONTINUOUS_FIRE residual cluster.
        && (MODELCONDITION_CONTINUOUS_FIRE_MEAN == MODELCONDITION_CONTINUOUS_FIRE_SLOW + 1)
        && (MODELCONDITION_CONTINUOUS_FIRE_FAST == MODELCONDITION_CONTINUOUS_FIRE_MEAN + 1)
        && {
            let mut names: Vec<&str> = MODEL_CONDITION_BIT_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// WeaponBonus residual table (Weapon.h TheWeaponBonusNames; ALLOW_DEMORALIZE off)
// ---------------------------------------------------------------------------

/// C++ `WEAPONBONUSCONDITION_COUNT` residual (ALLOW_DEMORALIZE off).
pub const WEAPONBONUSCONDITION_COUNT: u32 = 27;

/// Ordered C++ `TheWeaponBonusNames` residual (ALLOW_DEMORALIZE off → DEMORALIZED_OBSOLETE).
pub const WEAPON_BONUS_CONDITION_NAME_LIST: &[&str] = &[
    "GARRISONED",
    "HORDE",
    "CONTINUOUS_FIRE_MEAN",
    "CONTINUOUS_FIRE_FAST",
    "NATIONALISM",
    "PLAYER_UPGRADE",
    "DRONE_SPOTTING",
    "DEMORALIZED_OBSOLETE",
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

/// C++ WEAPONBONUSCONDITION_CONTINUOUS_FIRE_MEAN residual ordinal.
pub const WEAPON_BONUS_CONTINUOUS_FIRE_MEAN: u32 = 2;
/// C++ WEAPONBONUSCONDITION_CONTINUOUS_FIRE_FAST residual ordinal.
pub const WEAPON_BONUS_CONTINUOUS_FIRE_FAST: u32 = 3;
/// C++ WEAPONBONUSCONDITION_ENTHUSIASTIC residual ordinal (ALLOW_DEMORALIZE off).
pub const WEAPON_BONUS_ENTHUSIASTIC_ORDINAL: u32 = 8;
/// C++ WEAPONBONUSCONDITION_SUBLIMINAL residual ordinal.
pub const WEAPON_BONUS_SUBLIMINAL_ORDINAL: u32 = 15;
/// C++ WEAPONBONUSCONDITION_FRENZY_ONE residual ordinal.
pub const WEAPON_BONUS_FRENZY_ONE_ORDINAL: u32 = 24;

/// Lookup WeaponBonus condition name index residual.
pub fn weapon_bonus_condition_name_index(name: &str) -> Option<usize> {
    WEAPON_BONUS_CONDITION_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 82 honesty: WeaponBonus residual type table pack.
pub fn honesty_weapon_bonus_enum_table_wave82() -> bool {
    WEAPONBONUSCONDITION_COUNT == 27
        && WEAPON_BONUS_CONDITION_NAME_LIST.len() == 27
        && WEAPON_BONUS_CONDITION_NAME_LIST[0] == "GARRISONED"
        && WEAPON_BONUS_CONDITION_NAME_LIST[2] == "CONTINUOUS_FIRE_MEAN"
        && WEAPON_BONUS_CONDITION_NAME_LIST[3] == "CONTINUOUS_FIRE_FAST"
        && WEAPON_BONUS_CONDITION_NAME_LIST[5] == "PLAYER_UPGRADE"
        && WEAPON_BONUS_CONDITION_NAME_LIST[7] == "DEMORALIZED_OBSOLETE"
        && WEAPON_BONUS_CONDITION_NAME_LIST[8] == "ENTHUSIASTIC"
        && WEAPON_BONUS_CONDITION_NAME_LIST[15] == "SUBLIMINAL"
        && WEAPON_BONUS_CONDITION_NAME_LIST[22] == "TARGET_FAERIE_FIRE"
        && WEAPON_BONUS_CONDITION_NAME_LIST[23] == "FANATICISM"
        && WEAPON_BONUS_CONDITION_NAME_LIST[24] == "FRENZY_ONE"
        && WEAPON_BONUS_CONDITION_NAME_LIST[26] == "FRENZY_THREE"
        && WEAPON_BONUS_CONTINUOUS_FIRE_MEAN == 2
        && WEAPON_BONUS_CONTINUOUS_FIRE_FAST == 3
        && WEAPON_BONUS_ENTHUSIASTIC_ORDINAL == 8
        && WEAPON_BONUS_SUBLIMINAL_ORDINAL == 15
        && WEAPON_BONUS_FRENZY_ONE_ORDINAL == 24
        && weapon_bonus_condition_name_index("ENTHUSIASTIC") == Some(8)
        && weapon_bonus_condition_name_index("SUBLIMINAL") == Some(15)
        && weapon_bonus_condition_name_index("FRENZY_ONE") == Some(24)
        // No live DEMORALIZED residual name under ALLOW_DEMORALIZE off.
        && !WEAPON_BONUS_CONDITION_NAME_LIST
            .iter()
            .any(|&n| n == "DEMORALIZED")
        && {
            let mut names: Vec<&str> = WEAPON_BONUS_CONDITION_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// ObjectStatus / StatusBits residual table (ObjectStatusTypes.cpp)
// ---------------------------------------------------------------------------

/// C++ `OBJECT_STATUS_COUNT` residual (ObjectStatusTypes.h).
pub const OBJECT_STATUS_COUNT: u32 = 45;

/// Ordered C++ `ObjectStatusMaskType::s_bitNameList` residual (excluding NULL).
pub const OBJECT_STATUS_BIT_NAME_LIST: &[&str] = &[
    "NONE",
    "DESTROYED",
    "CAN_ATTACK",
    "UNDER_CONSTRUCTION",
    "UNSELECTABLE",
    "NO_COLLISIONS",
    "NO_ATTACK",
    "AIRBORNE_TARGET",
    "PARACHUTING",
    "REPULSOR",
    "HIJACKED",
    "AFLAME",
    "BURNED",
    "WET",
    "IS_FIRING_WEAPON",
    "IS_BRAKING",
    "STEALTHED",
    "DETECTED",
    "CAN_STEALTH",
    "SOLD",
    "UNDERGOING_REPAIR",
    "RECONSTRUCTING",
    "MASKED",
    "IS_ATTACKING",
    "USING_ABILITY",
    "IS_AIMING_WEAPON",
    "NO_ATTACK_FROM_AI",
    "IGNORING_STEALTH",
    "IS_CARBOMB",
    "DECK_HEIGHT_OFFSET",
    "STATUS_RIDER1",
    "STATUS_RIDER2",
    "STATUS_RIDER3",
    "STATUS_RIDER4",
    "STATUS_RIDER5",
    "STATUS_RIDER6",
    "STATUS_RIDER7",
    "STATUS_RIDER8",
    "FAERIE_FIRE",
    "KILLING_SELF",
    "REASSIGN_PARKING",
    "BOOBY_TRAPPED",
    "IMMOBILE",
    "DISGUISED",
    "DEPLOYED",
];

/// C++ OBJECT_STATUS_FAERIE_FIRE residual ordinal.
pub const OBJECT_STATUS_FAERIE_FIRE: u32 = 38;
/// C++ OBJECT_STATUS_IS_CARBOMB residual ordinal.
pub const OBJECT_STATUS_IS_CARBOMB: u32 = 28;
/// C++ OBJECT_STATUS_STEALTHED residual ordinal.
pub const OBJECT_STATUS_STEALTHED: u32 = 16;
/// C++ OBJECT_STATUS_DEPLOYED residual ordinal.
pub const OBJECT_STATUS_DEPLOYED: u32 = 44;

/// Lookup ObjectStatus bit-name index residual.
pub fn object_status_bit_name_index(name: &str) -> Option<usize> {
    OBJECT_STATUS_BIT_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 82 honesty: ObjectStatus / StatusBits residual table pack.
pub fn honesty_object_status_enum_table_wave82() -> bool {
    OBJECT_STATUS_COUNT == 45
        && OBJECT_STATUS_BIT_NAME_LIST.len() == 45
        && OBJECT_STATUS_BIT_NAME_LIST[0] == "NONE"
        && OBJECT_STATUS_BIT_NAME_LIST[1] == "DESTROYED"
        && OBJECT_STATUS_BIT_NAME_LIST[3] == "UNDER_CONSTRUCTION"
        && OBJECT_STATUS_BIT_NAME_LIST[14] == "IS_FIRING_WEAPON"
        && OBJECT_STATUS_BIT_NAME_LIST[16] == "STEALTHED"
        && OBJECT_STATUS_BIT_NAME_LIST[17] == "DETECTED"
        && OBJECT_STATUS_BIT_NAME_LIST[28] == "IS_CARBOMB"
        && OBJECT_STATUS_BIT_NAME_LIST[38] == "FAERIE_FIRE"
        && OBJECT_STATUS_BIT_NAME_LIST[41] == "BOOBY_TRAPPED"
        && OBJECT_STATUS_BIT_NAME_LIST[43] == "DISGUISED"
        && OBJECT_STATUS_BIT_NAME_LIST[44] == "DEPLOYED"
        && OBJECT_STATUS_FAERIE_FIRE == 38
        && OBJECT_STATUS_IS_CARBOMB == 28
        && OBJECT_STATUS_STEALTHED == 16
        && OBJECT_STATUS_DEPLOYED == 44
        && object_status_bit_name_index("FAERIE_FIRE") == Some(38)
        && object_status_bit_name_index("IS_CARBOMB") == Some(28)
        && object_status_bit_name_index("STEALTHED") == Some(16)
        && object_status_bit_name_index("DEPLOYED") == Some(44)
        // Rider residual cluster contiguous STATUS_RIDER1..8.
        && object_status_bit_name_index("STATUS_RIDER1") == Some(30)
        && object_status_bit_name_index("STATUS_RIDER8") == Some(37)
        && {
            let mut names: Vec<&str> = OBJECT_STATUS_BIT_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 82 residual pack
// ---------------------------------------------------------------------------

/// Wave 82 honesty: all five residual enum table peels.
pub fn honesty_enum_table_residual_pack_wave82() -> bool {
    honesty_damage_type_enum_table_wave82()
        && honesty_death_type_enum_table_wave82()
        && honesty_model_condition_enum_table_wave82()
        && honesty_weapon_bonus_enum_table_wave82()
        && honesty_object_status_enum_table_wave82()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_type_enum_table_wave82_honesty() {
        assert!(honesty_damage_type_enum_table_wave82());
        assert_eq!(damage_type_bit_name_index("STATUS"), Some(37));
    }

    #[test]
    fn death_type_enum_table_wave82_honesty() {
        assert!(honesty_death_type_enum_table_wave82());
        assert_eq!(death_type_name_index("POISONED_GAMMA"), Some(20));
    }

    #[test]
    fn model_condition_enum_table_wave82_honesty() {
        assert!(honesty_model_condition_enum_table_wave82());
        assert_eq!(
            model_condition_bit_name_index("CONTINUOUS_FIRE_FAST"),
            Some(86)
        );
        assert!(!MODEL_CONDITION_BIT_NAME_LIST.contains(&"SURRENDER"));
    }

    #[test]
    fn weapon_bonus_enum_table_wave82_honesty() {
        assert!(honesty_weapon_bonus_enum_table_wave82());
        assert_eq!(weapon_bonus_condition_name_index("FRENZY_THREE"), Some(26));
        assert_eq!(WEAPON_BONUS_ENTHUSIASTIC_ORDINAL, 8);
    }

    #[test]
    fn object_status_enum_table_wave82_honesty() {
        assert!(honesty_object_status_enum_table_wave82());
        assert_eq!(object_status_bit_name_index("FAERIE_FIRE"), Some(38));
    }

    #[test]
    fn enum_table_residual_pack_wave82_honesty() {
        assert!(honesty_enum_table_residual_pack_wave82());
    }
}
