// FILE: model_state.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/ModelState.h
// Author: Michael S. Booth, April 2001
//
// PARITY_NOTE: The C++ defines ModelConditionFlagType enum with exact
// discriminant values that are saved in save files.  The existing
// common::bit_flags::ModelConditionFlags struct provides the same indices
// as constants.  We define the enum here for type-safe usage.

pub const NUM_MODELCONDITION_DOOR_STATES: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum ModelConditionFlagType {
    Invalid = -1,

    Toppled = 0,
    FrontCrushed = 1,
    BackCrushed = 2,
    Damaged = 3,
    ReallyDamaged = 4,
    Rubble = 5,
    SpecialDamaged = 6,
    Night = 7,
    Snow = 8,
    Parachuting = 9,
    Garrisoned = 10,
    EnemyNear = 11,
    WeaponsetVeteran = 12,
    WeaponsetElite = 13,
    WeaponsetHero = 14,
    WeaponsetCrateupgradeOne = 15,
    WeaponsetCrateupgradeTwo = 16,
    WeaponsetPlayerUpgrade = 17,
    Door1Opening = 18,
    Door1Closing = 19,
    Door1WaitingOpen = 20,
    Door1WaitingToClose = 21,
    Door2Opening = 22,
    Door2Closing = 23,
    Door2WaitingOpen = 24,
    Door2WaitingToClose = 25,
    Door3Opening = 26,
    Door3Closing = 27,
    Door3WaitingOpen = 28,
    Door3WaitingToClose = 29,
    Door4Opening = 30,
    Door4Closing = 31,
    Door4WaitingOpen = 32,
    Door4WaitingToClose = 33,
    Attacking = 34,
    PreattackA = 35,
    FiringA = 36,
    BetweenFiringShotsA = 37,
    ReloadingA = 38,
    PreattackB = 39,
    FiringB = 40,
    BetweenFiringShotsB = 41,
    ReloadingB = 42,
    PreattackC = 43,
    FiringC = 44,
    BetweenFiringShotsC = 45,
    ReloadingC = 46,
    TurretRotate = 47,
    PostCollapse = 48,
    Moving = 49,
    Dying = 50,
    AwaitingConstruction = 51,
    PartiallyConstructed = 52,
    ActivelyBeingConstructed = 53,
    Prone = 54,
    Freefall = 55,
    ActivelyConstructing = 56,
    ConstructionComplete = 57,
    RadarExtending = 58,
    RadarUpgraded = 59,
    Panicking = 60,
    Aflame = 61,
    Smoldering = 62,
    Burned = 63,
    Docking = 64,
    DockingBeginning = 65,
    DockingActive = 66,
    DockingEnding = 67,
    Carrying = 68,
    Flooded = 69,
    Loaded = 70,
    JetAfterburner = 71,
    JetExhaust = 72,
    Packing = 73,
    Unpacking = 74,
    Deployed = 75,
    OverWater = 76,
    PowerPlantUpgraded = 77,
    Climbing = 78,
    Sold = 79,
    Rappelling = 80,
    Armed = 81,
    PowerPlantUpgrading = 82,
    SpecialCheering = 83,
    ContinuousFireSlow = 84,
    ContinuousFireMean = 85,
    ContinuousFireFast = 86,
    RaisingFlag = 87,
    Captured = 88,
    ExplodedFlailing = 89,
    ExplodedBouncing = 90,
    Splatted = 91,
    UsingWeaponA = 92,
    UsingWeaponB = 93,
    UsingWeaponC = 94,
    Preorder = 95,
    CenterToLeft = 96,
    LeftToCenter = 97,
    CenterToRight = 98,
    RightToCenter = 99,
    Rider1 = 100,
    Rider2 = 101,
    Rider3 = 102,
    Rider4 = 103,
    Rider5 = 104,
    Rider6 = 105,
    Rider7 = 106,
    Rider8 = 107,
    StunnedFlailing = 108,
    Stunned = 109,
    SecondLife = 110,
    Jammed = 111,
    ArmorsetCrateupgradeOne = 112,
    ArmorsetCrateupgradeTwo = 113,
    User1 = 114,
    User2 = 115,
    Disguised = 116,

    Count = 117,
}

impl Default for ModelConditionFlagType {
    fn default() -> Self {
        Self::Invalid
    }
}

impl ModelConditionFlagType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            -1 => Self::Invalid,
            0 => Self::Toppled,
            1 => Self::FrontCrushed,
            2 => Self::BackCrushed,
            3 => Self::Damaged,
            4 => Self::ReallyDamaged,
            5 => Self::Rubble,
            6 => Self::SpecialDamaged,
            7 => Self::Night,
            8 => Self::Snow,
            9 => Self::Parachuting,
            10 => Self::Garrisoned,
            11 => Self::EnemyNear,
            12 => Self::WeaponsetVeteran,
            13 => Self::WeaponsetElite,
            14 => Self::WeaponsetHero,
            15 => Self::WeaponsetCrateupgradeOne,
            16 => Self::WeaponsetCrateupgradeTwo,
            17 => Self::WeaponsetPlayerUpgrade,
            18 => Self::Door1Opening,
            19 => Self::Door1Closing,
            20 => Self::Door1WaitingOpen,
            21 => Self::Door1WaitingToClose,
            22 => Self::Door2Opening,
            23 => Self::Door2Closing,
            24 => Self::Door2WaitingOpen,
            25 => Self::Door2WaitingToClose,
            26 => Self::Door3Opening,
            27 => Self::Door3Closing,
            28 => Self::Door3WaitingOpen,
            29 => Self::Door3WaitingToClose,
            30 => Self::Door4Opening,
            31 => Self::Door4Closing,
            32 => Self::Door4WaitingOpen,
            33 => Self::Door4WaitingToClose,
            34 => Self::Attacking,
            35 => Self::PreattackA,
            36 => Self::FiringA,
            37 => Self::BetweenFiringShotsA,
            38 => Self::ReloadingA,
            39 => Self::PreattackB,
            40 => Self::FiringB,
            41 => Self::BetweenFiringShotsB,
            42 => Self::ReloadingB,
            43 => Self::PreattackC,
            44 => Self::FiringC,
            45 => Self::BetweenFiringShotsC,
            46 => Self::ReloadingC,
            47 => Self::TurretRotate,
            48 => Self::PostCollapse,
            49 => Self::Moving,
            50 => Self::Dying,
            51 => Self::AwaitingConstruction,
            52 => Self::PartiallyConstructed,
            53 => Self::ActivelyBeingConstructed,
            54 => Self::Prone,
            55 => Self::Freefall,
            56 => Self::ActivelyConstructing,
            57 => Self::ConstructionComplete,
            58 => Self::RadarExtending,
            59 => Self::RadarUpgraded,
            60 => Self::Panicking,
            61 => Self::Aflame,
            62 => Self::Smoldering,
            63 => Self::Burned,
            64 => Self::Docking,
            65 => Self::DockingBeginning,
            66 => Self::DockingActive,
            67 => Self::DockingEnding,
            68 => Self::Carrying,
            69 => Self::Flooded,
            70 => Self::Loaded,
            71 => Self::JetAfterburner,
            72 => Self::JetExhaust,
            73 => Self::Packing,
            74 => Self::Unpacking,
            75 => Self::Deployed,
            76 => Self::OverWater,
            77 => Self::PowerPlantUpgraded,
            78 => Self::Climbing,
            79 => Self::Sold,
            80 => Self::Rappelling,
            81 => Self::Armed,
            82 => Self::PowerPlantUpgrading,
            83 => Self::SpecialCheering,
            84 => Self::ContinuousFireSlow,
            85 => Self::ContinuousFireMean,
            86 => Self::ContinuousFireFast,
            87 => Self::RaisingFlag,
            88 => Self::Captured,
            89 => Self::ExplodedFlailing,
            90 => Self::ExplodedBouncing,
            91 => Self::Splatted,
            92 => Self::UsingWeaponA,
            93 => Self::UsingWeaponB,
            94 => Self::UsingWeaponC,
            95 => Self::Preorder,
            96 => Self::CenterToLeft,
            97 => Self::LeftToCenter,
            98 => Self::CenterToRight,
            99 => Self::RightToCenter,
            100 => Self::Rider1,
            101 => Self::Rider2,
            102 => Self::Rider3,
            103 => Self::Rider4,
            104 => Self::Rider5,
            105 => Self::Rider6,
            106 => Self::Rider7,
            107 => Self::Rider8,
            108 => Self::StunnedFlailing,
            109 => Self::Stunned,
            110 => Self::SecondLife,
            111 => Self::Jammed,
            112 => Self::ArmorsetCrateupgradeOne,
            113 => Self::ArmorsetCrateupgradeTwo,
            114 => Self::User1,
            115 => Self::User2,
            116 => Self::Disguised,
            _ => {
                log::warn!(
                    "ModelConditionFlagType::from_i32({v}) out of range, defaulting to Invalid"
                );
                Self::Invalid
            }
        }
    }

    pub fn to_index(self) -> usize {
        (self as i32) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_condition_saved_values_match_cpp_order() {
        assert_eq!(ModelConditionFlagType::Invalid as i32, -1);
        assert_eq!(ModelConditionFlagType::Toppled as i32, 0);
        assert_eq!(ModelConditionFlagType::Aflame as i32, 61);
        assert_eq!(ModelConditionFlagType::Disguised as i32, 116);
        assert_eq!(ModelConditionFlagType::Count as i32, 117);

        assert_eq!(
            ModelConditionFlagType::from_i32(-1),
            ModelConditionFlagType::Invalid
        );
        assert_eq!(
            ModelConditionFlagType::from_i32(61),
            ModelConditionFlagType::Aflame
        );
        assert_eq!(
            ModelConditionFlagType::from_i32(116),
            ModelConditionFlagType::Disguised
        );
        assert_eq!(
            ModelConditionFlagType::from_i32(117),
            ModelConditionFlagType::Invalid
        );
    }
}
