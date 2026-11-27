// ObjectStatus, DisabledType, weapons, and KindOf enums
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Enumeration types matching C++ definitions

pub use generals_sim::object::ObjectStatusTypes;

/// Disabled types (matching C++ DisabledType order).
///
/// Definition hoisted into `Common` (`common::thing::update_module`) alongside
/// `DisabledMaskType`, which its helpers take as an argument. Re-exported here
/// so the `crate::common::DisabledType` path stays stable.
pub use game_engine::common::thing::update_module::DisabledType;

/// Weapon set types (matching C++ WeaponSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSetType {
    Primary,
    Secondary,
    Tertiary,
    Passenger,
}

pub use generals_sim::weapon::WeaponSlotType;

/// Weapon lock types (matching C++ WeaponLockType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponLockType {
    None,
    Acquiring,
    Locked,
    LockedTemporarily,
}

bitflags! {
    /// Script-driven status overrides (`ObjectScriptStatusBit` in C++).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectScriptStatusBits: u8 {
        const DISABLED    = 0x01;
        const UNPOWERED   = 0x02;
        const UNSELLABLE  = 0x04;
        const UNSTEALTHED = 0x08;
        const TARGETABLE  = 0x10;
    }
}

pub use generals_sim::object::ObjectShroudStatus;

/// Radar priority levels (`RadarPriorityType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RadarPriorityType {
    Invalid = 0,
    NotOnRadar = 1,
    Structure = 2,
    Unit = 3,
    LocalUnitOnly = 4,
}

// Re-export canonical CommandSourceType from Common (matches C++ with #[repr(u32)])
pub use game_engine::common::game_common::CommandSourceType;

pub use game_engine::common::thing::ai_update_data::LocomotorSetType;

// Re-export AbleToAttackType helpers from Common (C++ bitmask semantics)
pub use game_engine::common::game_common::{
    AbleToAttackType, is_continued_attack, is_forced_attack,
};

/// Turret identifiers (`WhichTurretType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WhichTurretType {
    Invalid = -1,
    Main = 0,
    Alt = 1,
    MaxTurrets = 2,
}

/// Special power identifiers (`SpecialPowerType` in C++).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SpecialPowerType {
    SpecialInvalid = 0,
    SpecialDaisyCutter,
    SpecialParadropAmerica,
    SpecialCarpetBomb,
    SpecialClusterMines,
    SpecialEmpPulse,
    SpecialNapalmStrike,
    SpecialCashHack,
    SpecialNeutronMissile,
    SpecialSpySatellite,
    SpecialDefector,
    SpecialTerrorCell,
    SpecialAmbush,
    SpecialBlackMarketNuke,
    SpecialAnthraxBomb,
    SpecialScudStorm,
    SpecialDemoralizeObsolete,
    SpecialCrateDrop,
    SpecialA10ThunderboltStrike,
    SpecialDetonateDirtyNuke,
    SpecialArtilleryBarrage,
    SpecialMissileDefenderLaserGuidedMissiles,
    SpecialRemoteCharges,
    SpecialTimedCharges,
    SpecialHelixNapalmBomb,
    SpecialHackerDisableBuilding,
    SpecialTankHunterTntAttack,
    SpecialBlackLotusCaptureBuilding,
    SpecialBlackLotusDisableVehicleHack,
    SpecialBlackLotusStealCashHack,
    SpecialInfantryCaptureBuilding,
    SpecialRadarVanScan,
    SpecialSpyDrone,
    SpecialDisguiseAsVehicle,
    SpecialBoobyTrap,
    SpecialRepairVehicles,
    SpecialParticleUplinkCannon,
    SpecialCashBounty,
    SpecialChangeBattlePlans,
    SpecialCiaIntelligence,
    SpecialCleanupArea,
    SpecialLaunchBaikonurRocket,
    SpecialSpectreGunship,
    SpecialGpsScrambler,
    SpecialFrenzy,
    SpecialSneakAttack,
    SpecialChinaCarpetBomb,
    EarlySpecialChinaCarpetBomb,
    SpecialLeafletDrop,
    EarlySpecialLeafletDrop,
    EarlySpecialFrenzy,
    SpecialCommunicationsDownload,
    EarlySpecialRepairVehicles,
    SpecialTankParadrop,
    SupwSpecialParticleUplinkCannon,
    AirfSpecialDaisyCutter,
    NukeSpecialClusterMines,
    NukeSpecialNeutronMissile,
    AirfSpecialA10ThunderboltStrike,
    AirfSpecialSpectreGunship,
    InfaSpecialParadropAmerica,
    SlthSpecialGpsScrambler,
    AirfSpecialCarpetBomb,
    SuprSpecialCruiseMissile,
    LazrSpecialParticleUplinkCannon,
    SupwSpecialNeutronMissile,
    SpecialBattleshipBombardment,
    SpecialPowerCount,
}

impl SpecialPowerType {
    /// Convert from a numeric value (matches C++ casting with bounds check).
    /// Exhaustive match — no transmute of an unchecked discriminant.
    pub fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::SpecialInvalid,
            1 => Self::SpecialDaisyCutter,
            2 => Self::SpecialParadropAmerica,
            3 => Self::SpecialCarpetBomb,
            4 => Self::SpecialClusterMines,
            5 => Self::SpecialEmpPulse,
            6 => Self::SpecialNapalmStrike,
            7 => Self::SpecialCashHack,
            8 => Self::SpecialNeutronMissile,
            9 => Self::SpecialSpySatellite,
            10 => Self::SpecialDefector,
            11 => Self::SpecialTerrorCell,
            12 => Self::SpecialAmbush,
            13 => Self::SpecialBlackMarketNuke,
            14 => Self::SpecialAnthraxBomb,
            15 => Self::SpecialScudStorm,
            16 => Self::SpecialDemoralizeObsolete,
            17 => Self::SpecialCrateDrop,
            18 => Self::SpecialA10ThunderboltStrike,
            19 => Self::SpecialDetonateDirtyNuke,
            20 => Self::SpecialArtilleryBarrage,
            21 => Self::SpecialMissileDefenderLaserGuidedMissiles,
            22 => Self::SpecialRemoteCharges,
            23 => Self::SpecialTimedCharges,
            24 => Self::SpecialHelixNapalmBomb,
            25 => Self::SpecialHackerDisableBuilding,
            26 => Self::SpecialTankHunterTntAttack,
            27 => Self::SpecialBlackLotusCaptureBuilding,
            28 => Self::SpecialBlackLotusDisableVehicleHack,
            29 => Self::SpecialBlackLotusStealCashHack,
            30 => Self::SpecialInfantryCaptureBuilding,
            31 => Self::SpecialRadarVanScan,
            32 => Self::SpecialSpyDrone,
            33 => Self::SpecialDisguiseAsVehicle,
            34 => Self::SpecialBoobyTrap,
            35 => Self::SpecialRepairVehicles,
            36 => Self::SpecialParticleUplinkCannon,
            37 => Self::SpecialCashBounty,
            38 => Self::SpecialChangeBattlePlans,
            39 => Self::SpecialCiaIntelligence,
            40 => Self::SpecialCleanupArea,
            41 => Self::SpecialLaunchBaikonurRocket,
            42 => Self::SpecialSpectreGunship,
            43 => Self::SpecialGpsScrambler,
            44 => Self::SpecialFrenzy,
            45 => Self::SpecialSneakAttack,
            46 => Self::SpecialChinaCarpetBomb,
            47 => Self::EarlySpecialChinaCarpetBomb,
            48 => Self::SpecialLeafletDrop,
            49 => Self::EarlySpecialLeafletDrop,
            50 => Self::EarlySpecialFrenzy,
            51 => Self::SpecialCommunicationsDownload,
            52 => Self::EarlySpecialRepairVehicles,
            53 => Self::SpecialTankParadrop,
            54 => Self::SupwSpecialParticleUplinkCannon,
            55 => Self::AirfSpecialDaisyCutter,
            56 => Self::NukeSpecialClusterMines,
            57 => Self::NukeSpecialNeutronMissile,
            58 => Self::AirfSpecialA10ThunderboltStrike,
            59 => Self::AirfSpecialSpectreGunship,
            60 => Self::InfaSpecialParadropAmerica,
            61 => Self::SlthSpecialGpsScrambler,
            62 => Self::AirfSpecialCarpetBomb,
            63 => Self::SuprSpecialCruiseMissile,
            64 => Self::LazrSpecialParticleUplinkCannon,
            65 => Self::SupwSpecialNeutronMissile,
            66 => Self::SpecialBattleshipBombardment,
            67 => Self::SpecialPowerCount,
            _ => return None,
        })
    }
}

/// Weapon choice criteria (matching C++ WeaponChoiceCriteria)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponChoiceCriteria {
    Prefer,
    RequireToFire,
    PreferMostDamage,
}

/// Weapon bonus condition type (matching C++ WeaponBonusConditionType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponBonusConditionType {
    Invalid,
    Garrisoned,
    Horde,
    ContinuousFireMean,
    ContinuousFireFast,
    Nationalism,
    PlayerUpgrade,
    DroneSpotting,
    Demoralized,
    DemoralizedObsolete,
    Enthusiastic,
    Veteran,
    Elite,
    Hero,
    BattlePlanBombardment,
    BattlePlanHoldTheLine,
    BattlePlanSearchAndDestroy,
    Subliminal,
    SoloHumanEasy,
    SoloHumanNormal,
    SoloHumanHard,
    SoloAiEasy,
    SoloAiNormal,
    SoloAiHard,
    TargetFaerieFire,
    Fanaticism,
    FrenzyOne,
    FrenzyTwo,
    FrenzyThree,
    DroneSpotForStrike,
}

/// Armor set type (matching C++ ArmorSet.h:19-30 ArmorSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ArmorSetType {
    Veteran = 0,
    Elite = 1,
    Hero = 2,
    PlayerUpgrade = 3,
    WeakVersusBaseDefenses = 4,
    SecondLife = 5,
    CrateUpgradeOne = 6,
    CrateUpgradeTwo = 7,
}

/// Weapon status (matching C++ WeaponStatus)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    Ready,
    BetweenShots,
    Reloading,
    PreAttack,
}

/// Pathfind layer enum (matching C++ PathfindLayerEnum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    Bridge1 = 3,
    Bridge2 = 4,
    Bridge3 = 5,
    Bridge4 = 6,
    Bridge5 = 7,
    Bridge6 = 8,
    Bridge7 = 9,
    Bridge8 = 10,
    Bridge9 = 11,
    Bridge10 = 12,
    Bridge11 = 13,
    Bridge12 = 14,
    /// C++ `LAYER_WALL = 15`.
    Wall = 15,
    /// Leftover extras — not present in C++ PathfindLayerEnum.
    Tunnel = 16,
    Water = 17,
    Air = 18,
    Last = 19,
}

impl PathfindLayerEnum {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => PathfindLayerEnum::Invalid,
            1 => PathfindLayerEnum::Ground,
            2 => PathfindLayerEnum::Top,
            3 => PathfindLayerEnum::Bridge1,
            4 => PathfindLayerEnum::Bridge2,
            5 => PathfindLayerEnum::Bridge3,
            6 => PathfindLayerEnum::Bridge4,
            7 => PathfindLayerEnum::Bridge5,
            8 => PathfindLayerEnum::Bridge6,
            9 => PathfindLayerEnum::Bridge7,
            10 => PathfindLayerEnum::Bridge8,
            11 => PathfindLayerEnum::Bridge9,
            12 => PathfindLayerEnum::Bridge10,
            13 => PathfindLayerEnum::Bridge11,
            14 => PathfindLayerEnum::Bridge12,
            15 => PathfindLayerEnum::Wall,
            16 => PathfindLayerEnum::Tunnel,
            17 => PathfindLayerEnum::Water,
            18 => PathfindLayerEnum::Air,
            19 => PathfindLayerEnum::Last,
            _ => PathfindLayerEnum::Invalid,
        }
    }
}

/// Formation ID (matching C++ `FormationID`).
///
/// In the original engine this is an opaque, per-group identifier assigned by the AI system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormationID(u32);

impl FormationID {
    pub const NONE: FormationID = FormationID(0);

    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl Default for FormationID {
    fn default() -> Self {
        FormationID::NONE
    }
}

#[cfg(test)]
mod special_power_from_u32_tests {
    use super::SpecialPowerType;

    #[test]
    fn special_power_from_u32_is_exhaustive_and_rejects_oob() {
        assert_eq!(
            SpecialPowerType::from_u32(0),
            Some(SpecialPowerType::SpecialInvalid)
        );
        assert_eq!(
            SpecialPowerType::from_u32(8),
            Some(SpecialPowerType::SpecialNeutronMissile)
        );
        assert_eq!(
            SpecialPowerType::from_u32(66),
            Some(SpecialPowerType::SpecialBattleshipBombardment)
        );
        assert_eq!(
            SpecialPowerType::from_u32(67),
            Some(SpecialPowerType::SpecialPowerCount)
        );
        assert_eq!(SpecialPowerType::from_u32(68), None);
    }
}

pub use generals_sim::object::VeterancyLevel;
// Re-export canonical Relationship from Common (Enemies=0, Neutral=1, Allies=2 matching C++)
pub use game_engine::common::game_common::Relationship;

// Re-export canonical DamageType/DeathType from damage_system (38/21 C++-correct variants)
pub use crate::weapon::damage_system::{DamageType, DeathType};

pub use generals_sim::kind_of::classic::{KindOf, kindof_from_name};
