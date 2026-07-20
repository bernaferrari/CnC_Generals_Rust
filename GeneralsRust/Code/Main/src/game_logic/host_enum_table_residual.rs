//! Wave 82 + Wave 84: residual enum / bit-name table honesty packs.
//!
//! Freezes C++ ordered name tables used by INI parsers and save/load:
//!
//! Wave 82:
//! - DamageTypeFlags::s_bitNameList (Damage.cpp) — DAMAGE_NUM_TYPES **38**
//! - TheDeathNames (Damage.h DEFINE_DEATH_NAMES) — DEATH_NUM_TYPES **21**
//! - ModelConditionFlags::s_bitNameList (BitFlags.cpp) — MODELCONDITION_COUNT **117**
//!   (ALLOW_SURRENDER off — no SURRENDER bit between SOLD and RAPPELLING)
//! - TheWeaponBonusNames (Weapon.h, ALLOW_DEMORALIZE off) — COUNT **27**
//! - ObjectStatusMaskType::s_bitNameList (ObjectStatusTypes.cpp) — COUNT **45**
//!
//! Wave 84:
//! - KindOfMaskType::s_bitNameList (KindOf.cpp) — KINDOF_COUNT **116** (ALLOW_SURRENDER off)
//! - TheWeaponSlotTypeNames (WeaponSet.h) — WEAPONSLOT_COUNT **3**
//! - TheVeterancyNames (GameCommon.cpp) — LEVEL_COUNT **4**
//! - TheRelationshipNames (GameCommon.cpp) — ENEMIES/NEUTRAL/ALLIES **3**
//! - GeometryNames (Geometry.h) — GEOMETRY_NUM_TYPES **3**
//! - TheShadowNames (Shadow.h) — bit-name list **7**
//!
//! Fail-closed:
//! - Not full armor/weapon combat application of every discriminant
//! - Not full W3D MODELCONDITION anim draw matrix
//! - Not full ObjectStatus Xfer rebind / StatusBitsUpgrade matrix
//! - Not full KindOf mask runtime / WeaponSet fire matrix / Geometry collision
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

// ---------------------------------------------------------------------------
// Host body-damage → model-condition residual (C++ ActiveBody + Drawable)
// ---------------------------------------------------------------------------

/// C++ BodyDamageType residual for host Object visual condition.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum HostBodyDamageType {
    #[default]
    Pristine,
    Damaged,
    ReallyDamaged,
    Rubble,
}

impl HostBodyDamageType {
    #[inline]
    pub fn ordinal(self) -> u8 {
        match self {
            HostBodyDamageType::Pristine => 0,
            HostBodyDamageType::Damaged => 1,
            HostBodyDamageType::ReallyDamaged => 2,
            HostBodyDamageType::Rubble => 3,
        }
    }

    #[inline]
    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => HostBodyDamageType::Damaged,
            2 => HostBodyDamageType::ReallyDamaged,
            3 => HostBodyDamageType::Rubble,
            _ => HostBodyDamageType::Pristine,
        }
    }
}

/// C++ GlobalData unitDamagedThresh / unitReallyDamagedThresh defaults.
pub const HOST_UNIT_DAMAGED_THRESH: f32 = 0.5;
pub const HOST_UNIT_REALLY_DAMAGED_THRESH: f32 = 0.25;

/// ModelCondition bit indices residual (ALLOW_SURRENDER off list).
pub const MC_BIT_DAMAGED: u32 = 3;
pub const MC_BIT_REALLYDAMAGED: u32 = 4;
pub const MC_BIT_RUBBLE: u32 = 5;
pub const MC_BIT_ATTACKING: u32 = 34;
pub const MC_BIT_MOVING: u32 = 49;
/// C++ MODELCONDITION_DISGUISED residual bit index.
pub const MC_BIT_DISGUISED: u32 = 116;

pub const MC_BIT_DYING: u32 = 50;

/// C++ MODELCONDITION_STUNNED_FLAILING residual index (ALLOW_SURRENDER off).
pub const MC_BIT_STUNNED_FLAILING: u32 = 108;
/// C++ MODELCONDITION_STUNNED residual index.
pub const MC_BIT_STUNNED: u32 = 109;

/// C++ MODELCONDITION_FREEFALL residual index.
pub const MC_BIT_FREEFALL: u32 = 55;

/// C++ MODELCONDITION_SPLATTED residual index.
pub const MC_BIT_SPLATTED: u32 = 91;

/// C++ ActiveBody::calcDamageState residual (default thresholds).
pub fn host_calc_body_damage_state(health: f32, max_health: f32) -> HostBodyDamageType {
    if max_health <= 0.0 {
        return HostBodyDamageType::Pristine;
    }
    let ratio = health / max_health;
    if ratio > HOST_UNIT_DAMAGED_THRESH {
        HostBodyDamageType::Pristine
    } else if ratio > HOST_UNIT_REALLY_DAMAGED_THRESH {
        HostBodyDamageType::Damaged
    } else if ratio > 0.0 {
        HostBodyDamageType::ReallyDamaged
    } else {
        HostBodyDamageType::Rubble
    }
}

/// Clear DAMAGED/REALLYDAMAGED/RUBBLE and set the bit for `state` (C++ reactToBodyDamageStateChange).
pub fn host_apply_body_damage_model_bits(bits: u128, state: HostBodyDamageType) -> u128 {
    let mut b = bits;
    b &= !(1u128 << MC_BIT_DAMAGED);
    b &= !(1u128 << MC_BIT_REALLYDAMAGED);
    b &= !(1u128 << MC_BIT_RUBBLE);
    match state {
        HostBodyDamageType::Pristine => {}
        HostBodyDamageType::Damaged => b |= 1u128 << MC_BIT_DAMAGED,
        HostBodyDamageType::ReallyDamaged => b |= 1u128 << MC_BIT_REALLYDAMAGED,
        HostBodyDamageType::Rubble => b |= 1u128 << MC_BIT_RUBBLE,
    }
    b
}

pub fn host_model_condition_has(bits: u128, bit: u32) -> bool {
    (bits & (1u128 << bit)) != 0
}

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

/// C++ MODELCONDITION_CONSTRUCTION_COMPLETE residual bit index from name table.
pub fn construction_complete_model_bit() -> u32 {
    model_condition_bit_name_index("CONSTRUCTION_COMPLETE").unwrap_or(55) as u32
}

/// C++ MODELCONDITION_RADAR_EXTENDING residual bit index from name table.
pub fn radar_extending_model_bit() -> u32 {
    model_condition_bit_name_index("RADAR_EXTENDING").unwrap_or(56) as u32
}

/// C++ MODELCONDITION_RADAR_UPGRADED residual bit index from name table.
pub fn radar_upgraded_model_bit() -> u32 {
    model_condition_bit_name_index("RADAR_UPGRADED").unwrap_or(57) as u32
}

/// C++ MODELCONDITION_DOOR_1_OPENING residual bit.

/// C++ MODELCONDITION_AWAITING_CONSTRUCTION residual bit.
pub fn awaiting_construction_model_bit() -> u32 {
    model_condition_bit_name_index("AWAITING_CONSTRUCTION").unwrap_or(0) as u32
}
/// C++ MODELCONDITION_PARTIALLY_CONSTRUCTED residual bit.
pub fn partially_constructed_model_bit() -> u32 {
    model_condition_bit_name_index("PARTIALLY_CONSTRUCTED").unwrap_or(0) as u32
}
/// C++ MODELCONDITION_ACTIVELY_BEING_CONSTRUCTED residual bit.
pub fn actively_being_constructed_model_bit() -> u32 {
    model_condition_bit_name_index("ACTIVELY_BEING_CONSTRUCTED").unwrap_or(0) as u32
}

/// C++ MODELCONDITION_ACTIVELY_CONSTRUCTING residual bit (dozer/producer).
pub fn actively_constructing_model_bit() -> u32 {
    model_condition_bit_name_index("ACTIVELY_CONSTRUCTING").unwrap_or(0) as u32
}

/// C++ MODELCONDITION_SOLD residual bit.
pub fn sold_model_bit() -> u32 {
    model_condition_bit_name_index("SOLD").unwrap_or(0) as u32
}

/// C++ MODELCONDITION_CAPTURED residual bit index.
pub fn captured_model_bit() -> u32 {
    model_condition_bit_name_index("CAPTURED").unwrap_or(0) as u32
}

pub fn door_1_opening_model_bit() -> u32 {
    model_condition_bit_name_index("DOOR_1_OPENING").unwrap_or(0) as u32
}
/// C++ MODELCONDITION_DOOR_1_WAITING_OPEN residual bit.
pub fn door_1_waiting_open_model_bit() -> u32 {
    model_condition_bit_name_index("DOOR_1_WAITING_OPEN").unwrap_or(0) as u32
}
/// C++ MODELCONDITION_DOOR_1_CLOSING residual bit.
pub fn door_1_closing_model_bit() -> u32 {
    model_condition_bit_name_index("DOOR_1_CLOSING").unwrap_or(0) as u32
}

/// C++ MODELCONDITION_DOOR_1_WAITING_TO_CLOSE residual bit.
pub fn door_1_waiting_to_close_model_bit() -> u32 {
    model_condition_bit_name_index("DOOR_1_WAITING_TO_CLOSE").unwrap_or(0) as u32
}

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

    #[test]
    fn kindof_enum_table_wave84_honesty() {
        assert!(honesty_kindof_enum_table_wave84());
        assert_eq!(kindof_bit_name_index("FS_SUPERWEAPON"), Some(90));
        assert_eq!(KINDOF_COUNT, 116);
        assert!(!KINDOF_BIT_NAME_LIST.contains(&"PRISON"));
    }

    #[test]
    fn weapon_slot_enum_table_wave84_honesty() {
        assert!(honesty_weapon_slot_enum_table_wave84());
        assert_eq!(weapon_slot_type_name_index("TERTIARY"), Some(2));
    }

    #[test]
    fn veterancy_level_enum_table_wave84_honesty() {
        assert!(honesty_veterancy_level_enum_table_wave84());
        assert_eq!(veterancy_level_name_index("HEROIC"), Some(3));
        assert!(veterancy_level_name_index("ROOKIE").is_none());
    }

    #[test]
    fn relationship_enum_table_wave84_honesty() {
        assert!(honesty_relationship_enum_table_wave84());
        assert_eq!(relationship_name_index("ALLIES"), Some(2));
        assert_eq!(relationship_name_index("ENEMIES"), Some(0));
    }

    #[test]
    fn geometry_type_enum_table_wave84_honesty() {
        assert!(honesty_geometry_type_enum_table_wave84());
        assert_eq!(geometry_type_name_index("CYLINDER"), Some(1));
        assert_eq!(geometry_type_name_index("BOX"), Some(2));
    }

    #[test]
    fn shadow_type_enum_table_wave84_honesty() {
        assert!(honesty_shadow_type_enum_table_wave84());
        assert_eq!(shadow_type_bit_value("SHADOW_VOLUME"), Some(0x02));
        assert!(shadow_type_name_index("SHADOW_NONE").is_none());
    }

    #[test]
    fn enum_table_residual_pack_wave84_honesty() {
        assert!(honesty_enum_table_residual_pack_wave84());
    }
}

// ===========================================================================
// Wave 84: KindOf / WeaponSlot / Veterancy / Relationship / Geometry / Shadow
// ===========================================================================
//
// Freezes additional C++ ordered name tables used by INI parsers and save/load:
// - KindOfMaskType::s_bitNameList (KindOf.cpp) — KINDOF_COUNT **116** (ALLOW_SURRENDER off)
// - TheWeaponSlotTypeNames (WeaponSet.h) — WEAPONSLOT_COUNT **3**
// - TheVeterancyNames (GameCommon.cpp) — LEVEL_COUNT **4**
// - TheRelationshipNames (GameCommon.cpp) — ENEMIES/NEUTRAL/ALLIES **3**
// - GeometryNames (Geometry.h) — GEOMETRY_NUM_TYPES **3**
// - TheShadowNames (Shadow.h) — bit-name list **7** (SHADOW_NONE not named; bit 0 = DECAL)
//
// Fail-closed:
// - Not full KindOf mask runtime on every ThingTemplate
// - Not full WeaponSet fire/slot selection matrix
// - Not full veterancy XP thresholds / health bonus application
// - Not full relationship matrix Player/Team wiring
// - Not full GeometryInfo collision / partition residual
// - Not full Shadow volume/decal GPU draw residual
// - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// KindOf residual bit-name table (KindOf.cpp; ALLOW_SURRENDER off)
// ---------------------------------------------------------------------------

/// C++ `KINDOF_COUNT` residual with ALLOW_SURRENDER off (KindOf.h).
pub const KINDOF_COUNT: u32 = 116;

/// Ordered C++ `KindOfMaskType::s_bitNameList` residual (excluding trailing NULL).
///
/// PRISON / COLLECTS_PRISON_BOUNTY / POW_TRUCK / CAN_SURRENDER are ifdef'd out
/// under ALLOW_SURRENDER (off for retail ZH).
pub const KINDOF_BIT_NAME_LIST: &[&str] = &[
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

/// C++ KINDOF_STRUCTURE residual ordinal.
pub const KINDOF_STRUCTURE: u32 = 7;
/// C++ KINDOF_INFANTRY residual ordinal.
pub const KINDOF_INFANTRY: u32 = 8;
/// C++ KINDOF_VEHICLE residual ordinal.
pub const KINDOF_VEHICLE: u32 = 9;
/// C++ KINDOF_AIRCRAFT residual ordinal.
pub const KINDOF_AIRCRAFT: u32 = 10;
/// C++ KINDOF_COMMANDCENTER residual ordinal.
pub const KINDOF_COMMANDCENTER: u32 = 14;
/// C++ KINDOF_PROJECTILE residual ordinal.
pub const KINDOF_PROJECTILE: u32 = 22;
/// C++ KINDOF_NO_COLLIDE residual ordinal.
pub const KINDOF_NO_COLLIDE: u32 = 27;
/// C++ KINDOF_FS_FACTORY residual ordinal.
pub const KINDOF_FS_FACTORY: u32 = 58;
/// C++ KINDOF_HERO residual ordinal.
pub const KINDOF_HERO: u32 = 85;
/// C++ KINDOF_FS_SUPERWEAPON residual ordinal.
pub const KINDOF_FS_SUPERWEAPON: u32 = 90;
/// C++ KINDOF_BOOBY_TRAP residual ordinal.
pub const KINDOF_BOOBY_TRAP: u32 = 97;
/// C++ KINDOF_EMP_HARDENED residual ordinal.
pub const KINDOF_EMP_HARDENED: u32 = 112;
/// C++ KINDOF_IGNORE_DOCKING_BONES residual ordinal (last).
pub const KINDOF_IGNORE_DOCKING_BONES: u32 = 115;

/// Lookup KindOf bit-name index residual.
pub fn kindof_bit_name_index(name: &str) -> Option<usize> {
    KINDOF_BIT_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 84 honesty: full KindOf residual bit-name table pack.
pub fn honesty_kindof_enum_table_wave84() -> bool {
    KINDOF_COUNT == 116
        && KINDOF_BIT_NAME_LIST.len() == 116
        && KINDOF_BIT_NAME_LIST[0] == "OBSTACLE"
        && KINDOF_BIT_NAME_LIST[1] == "SELECTABLE"
        && KINDOF_BIT_NAME_LIST[7] == "STRUCTURE"
        && KINDOF_BIT_NAME_LIST[8] == "INFANTRY"
        && KINDOF_BIT_NAME_LIST[9] == "VEHICLE"
        && KINDOF_BIT_NAME_LIST[10] == "AIRCRAFT"
        && KINDOF_BIT_NAME_LIST[14] == "COMMANDCENTER"
        && KINDOF_BIT_NAME_LIST[22] == "PROJECTILE"
        && KINDOF_BIT_NAME_LIST[27] == "NO_COLLIDE"
        && KINDOF_BIT_NAME_LIST[50] == "MINE"
        && KINDOF_BIT_NAME_LIST[51] == "CLEANUP_HAZARD"
        && KINDOF_BIT_NAME_LIST[57] == "FS_POWER"
        && KINDOF_BIT_NAME_LIST[58] == "FS_FACTORY"
        && KINDOF_BIT_NAME_LIST[85] == "HERO"
        && KINDOF_BIT_NAME_LIST[90] == "FS_SUPERWEAPON"
        && KINDOF_BIT_NAME_LIST[97] == "BOOBY_TRAP"
        && KINDOF_BIT_NAME_LIST[112] == "EMP_HARDENED"
        && KINDOF_BIT_NAME_LIST[115] == "IGNORE_DOCKING_BONES"
        // ALLOW_SURRENDER residual absent (no PRISON between COMMANDCENTER and LINEBUILD).
        && !KINDOF_BIT_NAME_LIST.contains(&"PRISON")
        && !KINDOF_BIT_NAME_LIST.contains(&"CAN_SURRENDER")
        && KINDOF_BIT_NAME_LIST[14] == "COMMANDCENTER"
        && KINDOF_BIT_NAME_LIST[15] == "LINEBUILD"
        // Anchor constants match ordinals.
        && KINDOF_STRUCTURE == 7
        && KINDOF_INFANTRY == 8
        && KINDOF_VEHICLE == 9
        && KINDOF_AIRCRAFT == 10
        && KINDOF_COMMANDCENTER == 14
        && KINDOF_PROJECTILE == 22
        && KINDOF_NO_COLLIDE == 27
        && KINDOF_FS_FACTORY == 58
        && KINDOF_HERO == 85
        && KINDOF_FS_SUPERWEAPON == 90
        && KINDOF_BOOBY_TRAP == 97
        && KINDOF_EMP_HARDENED == 112
        && KINDOF_IGNORE_DOCKING_BONES == 115
        && kindof_bit_name_index("STRUCTURE") == Some(7)
        && kindof_bit_name_index("INFANTRY") == Some(8)
        && kindof_bit_name_index("FS_SUPERWEAPON") == Some(90)
        && kindof_bit_name_index("IGNORE_DOCKING_BONES") == Some(115)
        // FS_* residual cluster sample.
        && kindof_bit_name_index("FS_BARRACKS") == Some(104)
        && kindof_bit_name_index("FS_WARFACTORY") == Some(105)
        && kindof_bit_name_index("FS_AIRFIELD") == Some(106)
        && {
            let mut names: Vec<&str> = KINDOF_BIT_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// WeaponSlot residual table (WeaponSet.h TheWeaponSlotTypeNames)
// ---------------------------------------------------------------------------

/// C++ `WEAPONSLOT_COUNT` residual (GameType.h).
pub const WEAPONSLOT_COUNT: u32 = 3;

/// Ordered C++ `TheWeaponSlotTypeNames` residual (PRIMARY/SECONDARY/TERTIARY).
pub const WEAPON_SLOT_TYPE_NAME_LIST: &[&str] = &["PRIMARY", "SECONDARY", "TERTIARY"];

/// C++ PRIMARY_WEAPON residual ordinal.
pub const PRIMARY_WEAPON: u32 = 0;
/// C++ SECONDARY_WEAPON residual ordinal.
pub const SECONDARY_WEAPON: u32 = 1;
/// C++ TERTIARY_WEAPON residual ordinal.
pub const TERTIARY_WEAPON: u32 = 2;

/// Lookup WeaponSlotType name index residual.
pub fn weapon_slot_type_name_index(name: &str) -> Option<usize> {
    WEAPON_SLOT_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 84 honesty: WeaponSlot residual type table pack.
pub fn honesty_weapon_slot_enum_table_wave84() -> bool {
    WEAPONSLOT_COUNT == 3
        && WEAPON_SLOT_TYPE_NAME_LIST.len() == 3
        && WEAPON_SLOT_TYPE_NAME_LIST[0] == "PRIMARY"
        && WEAPON_SLOT_TYPE_NAME_LIST[1] == "SECONDARY"
        && WEAPON_SLOT_TYPE_NAME_LIST[2] == "TERTIARY"
        && PRIMARY_WEAPON == 0
        && SECONDARY_WEAPON == 1
        && TERTIARY_WEAPON == 2
        && weapon_slot_type_name_index("PRIMARY") == Some(0)
        && weapon_slot_type_name_index("SECONDARY") == Some(1)
        && weapon_slot_type_name_index("TERTIARY") == Some(2)
        && weapon_slot_type_name_index("primary") == Some(0)
        // Fail-closed: no fourth slot residual.
        && weapon_slot_type_name_index("QUATERNARY").is_none()
}

// ---------------------------------------------------------------------------
// Veterancy residual level table (GameCommon.cpp TheVeterancyNames)
// ---------------------------------------------------------------------------

/// C++ `LEVEL_COUNT` residual (GameCommon.h).
pub const VETERANCY_LEVEL_COUNT: u32 = 4;

/// Ordered C++ `TheVeterancyNames` residual (REGULAR/VETERAN/ELITE/HEROIC).
pub const VETERANCY_LEVEL_NAME_LIST: &[&str] = &["REGULAR", "VETERAN", "ELITE", "HEROIC"];

/// C++ LEVEL_REGULAR residual ordinal.
pub const LEVEL_REGULAR: u32 = 0;
/// C++ LEVEL_VETERAN residual ordinal.
pub const LEVEL_VETERAN: u32 = 1;
/// C++ LEVEL_ELITE residual ordinal.
pub const LEVEL_ELITE: u32 = 2;
/// C++ LEVEL_HEROIC residual ordinal.
pub const LEVEL_HEROIC: u32 = 3;

/// Lookup VeterancyLevel name index residual.
pub fn veterancy_level_name_index(name: &str) -> Option<usize> {
    VETERANCY_LEVEL_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 84 honesty: Veterancy residual level table pack.
pub fn honesty_veterancy_level_enum_table_wave84() -> bool {
    VETERANCY_LEVEL_COUNT == 4
        && VETERANCY_LEVEL_NAME_LIST.len() == 4
        && VETERANCY_LEVEL_NAME_LIST[0] == "REGULAR"
        && VETERANCY_LEVEL_NAME_LIST[1] == "VETERAN"
        && VETERANCY_LEVEL_NAME_LIST[2] == "ELITE"
        && VETERANCY_LEVEL_NAME_LIST[3] == "HEROIC"
        && LEVEL_REGULAR == 0
        && LEVEL_VETERAN == 1
        && LEVEL_ELITE == 2
        && LEVEL_HEROIC == 3
        && veterancy_level_name_index("REGULAR") == Some(0)
        && veterancy_level_name_index("VETERAN") == Some(1)
        && veterancy_level_name_index("ELITE") == Some(2)
        && veterancy_level_name_index("HEROIC") == Some(3)
        // Fail-closed: ROOKIE is not a C++ name (REGULAR is level 0).
        && veterancy_level_name_index("ROOKIE").is_none()
}

// ---------------------------------------------------------------------------
// Relationship residual table (GameCommon.cpp TheRelationshipNames)
// ---------------------------------------------------------------------------

/// C++ Relationship residual count (ENEMIES/NEUTRAL/ALLIES).
pub const RELATIONSHIP_COUNT: u32 = 3;

/// Ordered C++ `TheRelationshipNames` residual.
///
/// Note order is ENEMIES=0, NEUTRAL=1, ALLIES=2 (not alphabetical).
pub const RELATIONSHIP_NAME_LIST: &[&str] = &["ENEMIES", "NEUTRAL", "ALLIES"];

/// C++ ENEMIES residual ordinal.
pub const RELATIONSHIP_ENEMIES: u32 = 0;
/// C++ NEUTRAL residual ordinal.
pub const RELATIONSHIP_NEUTRAL: u32 = 1;
/// C++ ALLIES residual ordinal.
pub const RELATIONSHIP_ALLIES: u32 = 2;

/// Lookup Relationship name index residual.
pub fn relationship_name_index(name: &str) -> Option<usize> {
    RELATIONSHIP_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 84 honesty: Relationship residual table pack.
pub fn honesty_relationship_enum_table_wave84() -> bool {
    RELATIONSHIP_COUNT == 3
        && RELATIONSHIP_NAME_LIST.len() == 3
        && RELATIONSHIP_NAME_LIST[0] == "ENEMIES"
        && RELATIONSHIP_NAME_LIST[1] == "NEUTRAL"
        && RELATIONSHIP_NAME_LIST[2] == "ALLIES"
        && RELATIONSHIP_ENEMIES == 0
        && RELATIONSHIP_NEUTRAL == 1
        && RELATIONSHIP_ALLIES == 2
        && relationship_name_index("ENEMIES") == Some(0)
        && relationship_name_index("NEUTRAL") == Some(1)
        && relationship_name_index("ALLIES") == Some(2)
        // Fail-closed: NEUTRALS plural is not the C++ name (NEUTRAL singular).
        && relationship_name_index("NEUTRALS").is_none()
        && relationship_name_index("FRIENDLY").is_none()
}

// ---------------------------------------------------------------------------
// Geometry residual type table (Geometry.h GeometryNames)
// ---------------------------------------------------------------------------

/// C++ `GEOMETRY_NUM_TYPES` residual.
pub const GEOMETRY_NUM_TYPES: u32 = 3;

/// Ordered C++ `GeometryNames` residual (SPHERE/CYLINDER/BOX).
pub const GEOMETRY_TYPE_NAME_LIST: &[&str] = &["SPHERE", "CYLINDER", "BOX"];

/// C++ GEOMETRY_SPHERE residual ordinal.
pub const GEOMETRY_SPHERE: u32 = 0;
/// C++ GEOMETRY_CYLINDER residual ordinal.
pub const GEOMETRY_CYLINDER: u32 = 1;
/// C++ GEOMETRY_BOX residual ordinal.
pub const GEOMETRY_BOX: u32 = 2;

/// Lookup GeometryType name index residual.
pub fn geometry_type_name_index(name: &str) -> Option<usize> {
    GEOMETRY_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 84 honesty: Geometry residual type table pack.
pub fn honesty_geometry_type_enum_table_wave84() -> bool {
    GEOMETRY_NUM_TYPES == 3
        && GEOMETRY_TYPE_NAME_LIST.len() == 3
        && GEOMETRY_TYPE_NAME_LIST[0] == "SPHERE"
        && GEOMETRY_TYPE_NAME_LIST[1] == "CYLINDER"
        && GEOMETRY_TYPE_NAME_LIST[2] == "BOX"
        && GEOMETRY_SPHERE == 0
        && GEOMETRY_CYLINDER == 1
        && GEOMETRY_BOX == 2
        && geometry_type_name_index("SPHERE") == Some(0)
        && geometry_type_name_index("CYLINDER") == Some(1)
        && geometry_type_name_index("BOX") == Some(2)
        // Fail-closed: no capsule/mesh residual geometry type.
        && geometry_type_name_index("CAPSULE").is_none()
}

// ---------------------------------------------------------------------------
// Shadow residual type table (Shadow.h TheShadowNames)
// ---------------------------------------------------------------------------

/// C++ TheShadowNames residual count (bit-name list; SHADOW_NONE is not listed).
pub const SHADOW_TYPE_NAME_COUNT: u32 = 7;

/// Ordered C++ `TheShadowNames` residual (bit 0 = SHADOW_DECAL value 0x01).
///
/// parseBitString maps name index i → bit (1 << i). SHADOW_NONE (0) is absent.
pub const SHADOW_TYPE_NAME_LIST: &[&str] = &[
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

/// C++ SHADOW_NONE residual value (not in name list).
pub const SHADOW_NONE: u32 = 0x0000_0000;
/// C++ SHADOW_DECAL residual bit value.
pub const SHADOW_DECAL: u32 = 0x0000_0001;
/// C++ SHADOW_VOLUME residual bit value.
pub const SHADOW_VOLUME: u32 = 0x0000_0002;
/// C++ SHADOW_PROJECTION residual bit value.
pub const SHADOW_PROJECTION: u32 = 0x0000_0004;
/// C++ SHADOW_DYNAMIC_PROJECTION residual bit value.
pub const SHADOW_DYNAMIC_PROJECTION: u32 = 0x0000_0008;
/// C++ SHADOW_DIRECTIONAL_PROJECTION residual bit value.
pub const SHADOW_DIRECTIONAL_PROJECTION: u32 = 0x0000_0010;
/// C++ SHADOW_ALPHA_DECAL residual bit value.
pub const SHADOW_ALPHA_DECAL: u32 = 0x0000_0020;
/// C++ SHADOW_ADDITIVE_DECAL residual bit value.
pub const SHADOW_ADDITIVE_DECAL: u32 = 0x0000_0040;

/// Lookup ShadowType bit-name index residual (bit = 1 << index).
pub fn shadow_type_name_index(name: &str) -> Option<usize> {
    SHADOW_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Residual: convert TheShadowNames index to C++ ShadowType bit value.
pub fn shadow_type_bit_value(name: &str) -> Option<u32> {
    shadow_type_name_index(name).map(|i| 1u32 << i)
}

/// Wave 84 honesty: Shadow residual type table pack.
pub fn honesty_shadow_type_enum_table_wave84() -> bool {
    SHADOW_TYPE_NAME_COUNT == 7
        && SHADOW_TYPE_NAME_LIST.len() == 7
        && SHADOW_TYPE_NAME_LIST[0] == "SHADOW_DECAL"
        && SHADOW_TYPE_NAME_LIST[1] == "SHADOW_VOLUME"
        && SHADOW_TYPE_NAME_LIST[2] == "SHADOW_PROJECTION"
        && SHADOW_TYPE_NAME_LIST[3] == "SHADOW_DYNAMIC_PROJECTION"
        && SHADOW_TYPE_NAME_LIST[4] == "SHADOW_DIRECTIONAL_PROJECTION"
        && SHADOW_TYPE_NAME_LIST[5] == "SHADOW_ALPHA_DECAL"
        && SHADOW_TYPE_NAME_LIST[6] == "SHADOW_ADDITIVE_DECAL"
        // Bit values match C++ ShadowType enum.
        && SHADOW_NONE == 0
        && SHADOW_DECAL == 0x01
        && SHADOW_VOLUME == 0x02
        && SHADOW_PROJECTION == 0x04
        && SHADOW_DYNAMIC_PROJECTION == 0x08
        && SHADOW_DIRECTIONAL_PROJECTION == 0x10
        && SHADOW_ALPHA_DECAL == 0x20
        && SHADOW_ADDITIVE_DECAL == 0x40
        && shadow_type_bit_value("SHADOW_DECAL") == Some(0x01)
        && shadow_type_bit_value("SHADOW_VOLUME") == Some(0x02)
        && shadow_type_bit_value("SHADOW_ADDITIVE_DECAL") == Some(0x40)
        // SHADOW_NONE is not a named bit residual.
        && shadow_type_name_index("SHADOW_NONE").is_none()
        && {
            let mut names: Vec<&str> = SHADOW_TYPE_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 84 residual pack
// ---------------------------------------------------------------------------

/// Wave 84 honesty: all six residual type-name table peels.
pub fn honesty_enum_table_residual_pack_wave84() -> bool {
    honesty_kindof_enum_table_wave84()
        && honesty_weapon_slot_enum_table_wave84()
        && honesty_veterancy_level_enum_table_wave84()
        && honesty_relationship_enum_table_wave84()
        && honesty_geometry_type_enum_table_wave84()
        && honesty_shadow_type_enum_table_wave84()
}
