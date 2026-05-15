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
    Aframe = 61,
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
        Self::Toppled
    }
}

impl ModelConditionFlagType {
    pub fn from_i32(v: i32) -> Self {
        match v {
            -1 => Self::Invalid,
            0..=116 => unsafe { std::mem::transmute(v) },
            _ => {
                log::warn!("ModelConditionFlagType::from_i32({v}) out of range, defaulting to Toppled");
                Self::Toppled
            }
        }
    }

    pub fn to_index(self) -> usize {
        (self as i32) as usize
    }
}
