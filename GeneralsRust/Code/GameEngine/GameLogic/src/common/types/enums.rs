// ObjectStatus, DisabledType, weapons, and KindOf enums
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Enumeration types matching C++ definitions

/// Object status types (matching C++ ObjectStatusTypes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ObjectStatusTypes {
    None = 0,
    Destroyed,
    CanAttack,
    UnderConstruction,
    Unselectable,
    NoCollisions,
    NoAttack,
    AirborneTarget,
    Parachuting,
    Repulsor,
    Hijacked,
    Aflame,
    Burned,
    Wet,
    IsFiringWeapon,
    Braking,
    Stealthed,
    Detected,
    CanStealth,
    Sold,
    UndergoingRepair,
    Reconstructing,
    Masked,
    IsAttacking,
    IsUsingAbility,
    IsAimingWeapon,
    NoAttackFromAi,
    IgnoringStealth,
    IsCarBomb,
    DeckHeightOffset,
    Rider1,
    Rider2,
    Rider3,
    Rider4,
    Rider5,
    Rider6,
    Rider7,
    Rider8,
    FaerieFire,
    MissileKillingSelf,
    ReassignParking,
    BoobyTrapped,
    Immobile,
    Disguised,
    Deployed,
}

impl ObjectStatusTypes {
    /// Convert from a raw integer value, defaulting to `ObjectStatusTypes::None`.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => ObjectStatusTypes::None,
            1 => ObjectStatusTypes::Destroyed,
            2 => ObjectStatusTypes::CanAttack,
            3 => ObjectStatusTypes::UnderConstruction,
            4 => ObjectStatusTypes::Unselectable,
            5 => ObjectStatusTypes::NoCollisions,
            6 => ObjectStatusTypes::NoAttack,
            7 => ObjectStatusTypes::AirborneTarget,
            8 => ObjectStatusTypes::Parachuting,
            9 => ObjectStatusTypes::Repulsor,
            10 => ObjectStatusTypes::Hijacked,
            11 => ObjectStatusTypes::Aflame,
            12 => ObjectStatusTypes::Burned,
            13 => ObjectStatusTypes::Wet,
            14 => ObjectStatusTypes::IsFiringWeapon,
            15 => ObjectStatusTypes::Braking,
            16 => ObjectStatusTypes::Stealthed,
            17 => ObjectStatusTypes::Detected,
            18 => ObjectStatusTypes::CanStealth,
            19 => ObjectStatusTypes::Sold,
            20 => ObjectStatusTypes::UndergoingRepair,
            21 => ObjectStatusTypes::Reconstructing,
            22 => ObjectStatusTypes::Masked,
            23 => ObjectStatusTypes::IsAttacking,
            24 => ObjectStatusTypes::IsUsingAbility,
            25 => ObjectStatusTypes::IsAimingWeapon,
            26 => ObjectStatusTypes::NoAttackFromAi,
            27 => ObjectStatusTypes::IgnoringStealth,
            28 => ObjectStatusTypes::IsCarBomb,
            29 => ObjectStatusTypes::DeckHeightOffset,
            30 => ObjectStatusTypes::Rider1,
            31 => ObjectStatusTypes::Rider2,
            32 => ObjectStatusTypes::Rider3,
            33 => ObjectStatusTypes::Rider4,
            34 => ObjectStatusTypes::Rider5,
            35 => ObjectStatusTypes::Rider6,
            36 => ObjectStatusTypes::Rider7,
            37 => ObjectStatusTypes::Rider8,
            38 => ObjectStatusTypes::FaerieFire,
            39 => ObjectStatusTypes::MissileKillingSelf,
            40 => ObjectStatusTypes::ReassignParking,
            41 => ObjectStatusTypes::BoobyTrapped,
            42 => ObjectStatusTypes::Immobile,
            43 => ObjectStatusTypes::Disguised,
            44 => ObjectStatusTypes::Deployed,
            _ => ObjectStatusTypes::None,
        }
    }

    // Legacy C++-style aliases used by in-progress ported call sites.
    pub const OBJECT_STATUS_IS_ATTACKING: ObjectStatusTypes = ObjectStatusTypes::IsAttacking;
}

/// Disabled types (matching C++ DisabledType order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledType {
    DisabledDefault,
    DisabledHacked,
    DisabledEmp,
    Held,
    Paralyzed,
    DisabledUnmanned,
    DisabledUnderpowered,
    DisabledFreefall,
    DisabledAwestruck,
    DisabledBrainwashed,
    DisabledSubdued,
    DisabledScriptDisabled,
    DisabledScriptUnderpowered,
    DisabledAny,
    Unmanned, // Alias for DisabledUnmanned
}

/// Weapon set types (matching C++ WeaponSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSetType {
    Primary,
    Secondary,
    Tertiary,
    Passenger,
}

/// Weapon slot types (matching C++ WeaponSlotType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

impl WeaponSlotType {
    /// Convert from u32 value (matches C++ casting)
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(WeaponSlotType::Primary),
            1 => Some(WeaponSlotType::Secondary),
            2 => Some(WeaponSlotType::Tertiary),
            _ => None,
        }
    }
}

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

/// Object shroud state (`ObjectShroudStatus` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjectShroudStatus {
    Invalid = 0,
    Clear = 1,
    PartialClear = 2,
    Fogged = 3,
    Shrouded = 4,
    InvalidButPreviousValid = 5,
}

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

/// Locomotor set selection (`LocomotorSetType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum LocomotorSetType {
    Invalid = -1,
    Normal = 0,
    NormalUpgraded = 1,
    Freefall = 2,
    Wander = 3,
    Panic = 4,
    Taxiing = 5,
    Supersonic = 6,
    Sluggish = 7,
}

// Re-export AbleToAttackType helpers from Common (C++ bitmask semantics)
pub use game_engine::common::game_common::{
    is_continued_attack, is_forced_attack, AbleToAttackType,
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

/// Armor set type (matching C++ ArmorSetType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSetType {
    Default,
    Veteran,
    Elite,
    Hero,
    Player,
    CrateUpgradeOne,
    CrateUpgradeTwo,
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
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    Bridge1 = 3,
    Bridge2 = 4,
    Bridge3 = 5,
    Bridge4 = 6,
    Wall = 7,
    Tunnel = 8,
    Water = 9,
    Air = 10,
    Last = 11, // Used for array bounds
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
            7 => PathfindLayerEnum::Wall,
            8 => PathfindLayerEnum::Tunnel,
            9 => PathfindLayerEnum::Water,
            10 => PathfindLayerEnum::Air,
            11 => PathfindLayerEnum::Last,
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
        let impl_src = include_str!("enums.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(
            !impl_src.contains("mem::transmute"),
            "SpecialPowerType::from_u32 must not transmute"
        );
    }
}

/// Veterancy level (matching C++ VeterancyLevel)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl VeterancyLevel {
    pub fn saturating_add_levels(self, delta: i32) -> Self {
        let raw = self as i32;
        let min = VeterancyLevel::Regular as i32;
        let max = VeterancyLevel::Heroic as i32;
        let clamped = (raw + delta).clamp(min, max);
        match clamped {
            0 => VeterancyLevel::Regular,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            _ => VeterancyLevel::Heroic,
        }
    }
}

impl fmt::Display for VeterancyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            VeterancyLevel::Regular => "Regular",
            VeterancyLevel::Veteran => "Veteran",
            VeterancyLevel::Elite => "Elite",
            VeterancyLevel::Heroic => "Heroic",
        };
        write!(f, "{}", label)
    }
}

// Re-export canonical Relationship from Common (Enemies=0, Neutral=1, Allies=2 matching C++)
pub use game_engine::common::game_common::Relationship;

// Re-export canonical DamageType/DeathType from damage_system (38/21 C++-correct variants)
pub use crate::weapon::damage_system::{DamageType, DeathType};

/// Kind of classifications for objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindOf {
    Selectable,
    Unit,
    Building,
    Vehicle,
    Infantry,
    Aircraft,
    Drone,
    CliffJumper,
    Structure,
    Weapon,
    Projectile,
    CanSeeThrough,
    AlwaysSelectable,
    Crate,
    ResourceNode,
    SupplySourceOnPreview,
    SupplySource,
    TechBuilding,
    Powered,
    ProducedAtHelipad,
    Bridge,
    Barrier,
    Civilian,
    Destructible,
    CanCrossBridges,
    Amphibious,
    AmphibiousTransport,
    Transport,
    CanCapture,
    Saboteur,
    Hacker,
    Hero,
    KeyStructure,
    CommandCenter,
    Prison,
    CollectsPrisonBounty,
    PowTruck,
    PowerPlant,
    Refinery,
    Factory,
    Defense,
    Shrubbery,
    Dozer,
    Harvester,
    Hulk,
    Salvager,
    WeaponSalvager,
    ArmorSalvager,
    AircraftCarrier,
    FSBarracks,
    FSWarfactory,
    FSAirfield,
    FSInternetCenter,
    FSPower,
    FSBaseDefense,
    FSSupplyDropzone,
    FSSupplyCenter,
    FSSuperweapon,
    FSStrategyCenter,
    FSFake,
    CountsForVictory,
    Mine,
    CleanupHazard,
    HealPad,
    WaveGuide,
    BridgeTower,
    Immobile,
    BoobyTrap,
    Disguiser,
    PortableStructure,
    CanRappel,
    CanBeRepulsed,
    EmpHardened,
    SpawnsAreTheWeapons,
    IgnoreDockingBones,
    CanSurrender,
    RepairPad,
    RejectUnmanned,
    IgnoredInGui,
    MobNexus,
    Capturable,
    ImmuneToCapture,
    CashGenerator,
    RebuildHole,
    FSTechnology,
    NoGarrison,
    Boat,
    GarrisonableUntilDestroyed,
    Obstacle,                   // KINDOF_OBSTACLE (bit 0)
    CanAttack,                  // KINDOF_CAN_ATTACK
    StickToTerrainSlope,        // KINDOF_STICK_TO_TERRAIN_SLOPE
    CanCastReflections,         // KINDOF_CAN_CAST_REFLECTIONS
    HugeVehicle,                // KINDOF_HUGE_VEHICLE
    LineBuild,                  // KINDOF_LINEBUILD
    Preload,                    // KINDOF_PRELOAD
    NoCollide,                  // KINDOF_NO_COLLIDE
    StealthGarrison,            // KINDOF_STEALTH_GARRISON
    DrawableOnly,               // KINDOF_DRAWABLE_ONLY
    Score,                      // KINDOF_SCORE
    ScoreCreate,                // KINDOF_SCORE_CREATE
    ScoreDestroy,               // KINDOF_SCORE_DESTROY
    NoHealIcon,                 // KINDOF_NO_HEAL_ICON
    Parachutable,               // KINDOF_PARACHUTABLE
    SmallMissile,               // KINDOF_SMALL_MISSILE
    AlwaysVisible,              // KINDOF_ALWAYS_VISIBLE
    Unattackable,               // KINDOF_UNATTACKABLE
    AttackNeedsLineOfSight,     // KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT
    WalkOnTopOfWall,            // KINDOF_WALK_ON_TOP_OF_WALL
    DefensiveWall,              // KINDOF_DEFENSIVE_WALL
    AircraftPathAround,         // KINDOF_AIRCRAFT_PATH_AROUND
    LowOverlappable,            // KINDOF_LOW_OVERLAPPABLE
    ForceAttackable,            // KINDOF_FORCEATTACKABLE
    AutoRallypoint,             // KINDOF_AUTO_RALLYPOINT
    MoneyHacker,                // KINDOF_MONEY_HACKER
    BallisticMissile,           // KINDOF_BALLISTIC_MISSILE
    ClickThrough,               // KINDOF_CLICK_THROUGH
    ShowPortraitWhenControlled, // KINDOF_SHOW_PORTRAIT_WHEN_CONTROLLED
    CannotBuildNearSupplies,    // KINDOF_CANNOT_BUILD_NEAR_SUPPLIES
    RevealToAll,                // KINDOF_REVEAL_TO_ALL
    IgnoresSelectAll,           // KINDOF_IGNORES_SELECT_ALL
    DontAutoCrushInfantry,      // KINDOF_DONT_AUTO_CRUSH_INFANTRY
    FsBlackMarket,              // KINDOF_FS_BLACK_MARKET
    FsAdvancedTech,             // KINDOF_FS_ADVANCED_TECH
    RevealsEnemyPaths,          // KINDOF_REVEALS_ENEMY_PATHS
    NoSelect,                   // KINDOF_NO_SELECT
    CannotRetaliate,            // KINDOF_CANNOT_RETALIATE
    TechBaseDefense,            // KINDOF_TECH_BASE_DEFENSE
    Demotrap,                   // KINDOF_DEMOTRAP
    ConservativeBuilding,       // KINDOF_CONSERVATIVE_BUILDING
    BlastCrater,                // KINDOF_BLAST_CRATER
    Prop,                       // KINDOF_PROP
    OptimizedTree,              // KINDOF_OPTIMIZED_TREE
    LandmarkBridge,             // KINDOF_LANDMARK_BRIDGE
    WaveEffect,                 // KINDOF_WAVE_EFFECT
    ClearedByBuild,             // KINDOF_CLEARED_BY_BUILD
    Parachute,                  // KINDOF_PARACHUTE
}

impl KindOf {
    /// Legacy script alias used by original C++ script conditions.
    pub const Inert: KindOf = KindOf::Immobile;
    /// Legacy script alias used by original C++ script actions.
    pub const CanRepair: KindOf = KindOf::RepairPad;
}

/// Resolve a KindOf flag name (as written in INI files) to its enum variant.
///
/// C++ Reference: KindOfMaskType::parseFromINI reads pipe-separated names like
/// `"SELECTABLE STRUCTURE CAN_ATTACK"` and OR's the corresponding bits.
///
/// The names here match the C++ INI token names exactly (uppercase, underscores).
/// We also accept mixed-case for robustness.
pub fn kindof_from_name(name: &str) -> Option<KindOf> {
    // Normalise: strip leading/trailing whitespace, uppercase for comparison
    let upper = name.trim().to_ascii_uppercase();
    match upper.as_str() {
        "SELECTABLE" => Some(KindOf::Selectable),
        "INFANTRY" => Some(KindOf::Infantry),
        "VEHICLE" => Some(KindOf::Vehicle),
        "STRUCTURE" => Some(KindOf::Structure),
        "AIRCRAFT" => Some(KindOf::Aircraft),
        "UNIT" => Some(KindOf::Unit),
        "DRONE" => Some(KindOf::Drone),
        "CLIFF_JUMPER" => Some(KindOf::CliffJumper),
        "WEAPON" => Some(KindOf::Weapon),
        "PROJECTILE" => Some(KindOf::Projectile),
        "CAN_SEE_THROUGH" => Some(KindOf::CanSeeThrough),
        "ALWAYS_SELECTABLE" => Some(KindOf::AlwaysSelectable),
        "CRATE" => Some(KindOf::Crate),
        "RESOURCE_NODE" => Some(KindOf::ResourceNode),
        "SUPPLY_SOURCE_ON_PREVIEW" => Some(KindOf::SupplySourceOnPreview),
        "SUPPLY_SOURCE" => Some(KindOf::SupplySource),
        "TECH_BUILDING" => Some(KindOf::TechBuilding),
        "POWERED" => Some(KindOf::Powered),
        "PRODUCED_AT_HELIPAD" => Some(KindOf::ProducedAtHelipad),
        "BRIDGE" => Some(KindOf::Bridge),
        "BARRIER" => Some(KindOf::Barrier),
        "CIVILIAN" => Some(KindOf::Civilian),
        "DESTRUCTIBLE" => Some(KindOf::Destructible),
        "CAN_CROSS_BRIDGES" => Some(KindOf::CanCrossBridges),
        "AMPHIBIOUS" => Some(KindOf::Amphibious),
        "AMPHIBIOUS_TRANSPORT" => Some(KindOf::AmphibiousTransport),
        "TRANSPORT" => Some(KindOf::Transport),
        "CAN_CAPTURE" => Some(KindOf::CanCapture),
        "SABOTEUR" => Some(KindOf::Saboteur),
        "HACKER" => Some(KindOf::Hacker),
        "HERO" => Some(KindOf::Hero),
        "KEY_STRUCTURE" => Some(KindOf::KeyStructure),
        "COMMAND_CENTER" => Some(KindOf::CommandCenter),
        "PRISON" => Some(KindOf::Prison),
        "COLLECTS_PRISON_BOUNTY" => Some(KindOf::CollectsPrisonBounty),
        "POW_TRUCK" => Some(KindOf::PowTruck),
        "POWER_PLANT" => Some(KindOf::PowerPlant),
        "REFINERY" => Some(KindOf::Refinery),
        "FACTORY" => Some(KindOf::Factory),
        "DEFENSE" => Some(KindOf::Defense),
        "SHRUBBERY" => Some(KindOf::Shrubbery),
        "DOZER" => Some(KindOf::Dozer),
        "HARVESTER" => Some(KindOf::Harvester),
        "HULK" => Some(KindOf::Hulk),
        "SALVAGER" => Some(KindOf::Salvager),
        "WEAPON_SALVAGER" => Some(KindOf::WeaponSalvager),
        "ARMOR_SALVAGER" => Some(KindOf::ArmorSalvager),
        "AIRCRAFT_CARRIER" => Some(KindOf::AircraftCarrier),
        "FS_BARRACKS" => Some(KindOf::FSBarracks),
        "FS_WARFACTORY" => Some(KindOf::FSWarfactory),
        "FS_AIRFIELD" => Some(KindOf::FSAirfield),
        "FS_INTERNET_CENTER" => Some(KindOf::FSInternetCenter),
        "FS_POWER" => Some(KindOf::FSPower),
        "FS_BASE_DEFENSE" => Some(KindOf::FSBaseDefense),
        "FS_SUPPLY_DROPZONE" => Some(KindOf::FSSupplyDropzone),
        "FS_SUPPLY_CENTER" => Some(KindOf::FSSupplyCenter),
        "FS_SUPERWEAPON" => Some(KindOf::FSSuperweapon),
        "FS_STRATEGY_CENTER" => Some(KindOf::FSStrategyCenter),
        "FS_FAKE" => Some(KindOf::FSFake),
        "FS_BLACK_MARKET" => Some(KindOf::FsBlackMarket),
        "FS_ADVANCED_TECH" => Some(KindOf::FsAdvancedTech),
        "FS_TECHNOLOGY" => Some(KindOf::FSTechnology),
        "COUNTS_FOR_VICTORY" => Some(KindOf::CountsForVictory),
        "MINE" => Some(KindOf::Mine),
        "CLEANUP_HAZARD" => Some(KindOf::CleanupHazard),
        "HEAL_PAD" => Some(KindOf::HealPad),
        "WAVE_GUIDE" => Some(KindOf::WaveGuide),
        "BRIDGE_TOWER" => Some(KindOf::BridgeTower),
        "IMMOBILE" | "INERT" => Some(KindOf::Immobile),
        "BOOBY_TRAP" => Some(KindOf::BoobyTrap),
        "DISGUISER" => Some(KindOf::Disguiser),
        "PORTABLE_STRUCTURE" => Some(KindOf::PortableStructure),
        "CAN_RAPPEL" => Some(KindOf::CanRappel),
        "CAN_BE_REPULSED" => Some(KindOf::CanBeRepulsed),
        "EMP_HARDENED" => Some(KindOf::EmpHardened),
        "SPAWNS_ARE_THE_WEAPONS" => Some(KindOf::SpawnsAreTheWeapons),
        "IGNORE_DOCKING_BONES" => Some(KindOf::IgnoreDockingBones),
        "CAN_SURRENDER" => Some(KindOf::CanSurrender),
        "REPAIR_PAD" | "CAN_REPAIR" => Some(KindOf::RepairPad),
        "REJECT_UNMANNED" => Some(KindOf::RejectUnmanned),
        "IGNORED_IN_GUI" => Some(KindOf::IgnoredInGui),
        "MOB_NEXUS" => Some(KindOf::MobNexus),
        "CAPTURABLE" => Some(KindOf::Capturable),
        "IMMUNE_TO_CAPTURE" => Some(KindOf::ImmuneToCapture),
        "CASH_GENERATOR" => Some(KindOf::CashGenerator),
        "REBUILD_HOLE" => Some(KindOf::RebuildHole),
        "NO_GARRISON" => Some(KindOf::NoGarrison),
        "BOAT" => Some(KindOf::Boat),
        "GARRISONABLE_UNTIL_DESTROYED" => Some(KindOf::GarrisonableUntilDestroyed),
        "OBSTACLE" => Some(KindOf::Obstacle),
        "CAN_ATTACK" => Some(KindOf::CanAttack),
        "STICK_TO_TERRAIN_SLOPE" => Some(KindOf::StickToTerrainSlope),
        "CAN_CAST_REFLECTIONS" => Some(KindOf::CanCastReflections),
        "HUGE_VEHICLE" => Some(KindOf::HugeVehicle),
        "LINEBUILD" | "LINE_BUILD" => Some(KindOf::LineBuild),
        "PRELOAD" => Some(KindOf::Preload),
        "NO_COLLIDE" => Some(KindOf::NoCollide),
        "STEALTH_GARRISON" => Some(KindOf::StealthGarrison),
        "DRAWABLE_ONLY" => Some(KindOf::DrawableOnly),
        "SCORE" => Some(KindOf::Score),
        "SCORE_CREATE" => Some(KindOf::ScoreCreate),
        "SCORE_DESTROY" => Some(KindOf::ScoreDestroy),
        "NO_HEAL_ICON" => Some(KindOf::NoHealIcon),
        "PARACHUTABLE" => Some(KindOf::Parachutable),
        "SMALL_MISSILE" => Some(KindOf::SmallMissile),
        "ALWAYS_VISIBLE" => Some(KindOf::AlwaysVisible),
        "UNATTACKABLE" => Some(KindOf::Unattackable),
        "ATTACK_NEEDS_LINE_OF_SIGHT" => Some(KindOf::AttackNeedsLineOfSight),
        "WALK_ON_TOP_OF_WALL" => Some(KindOf::WalkOnTopOfWall),
        "DEFENSIVE_WALL" => Some(KindOf::DefensiveWall),
        "AIRCRAFT_PATH_AROUND" => Some(KindOf::AircraftPathAround),
        "LOW_OVERLAPPABLE" => Some(KindOf::LowOverlappable),
        "FORCEATTACKABLE" | "FORCE_ATTACKABLE" => Some(KindOf::ForceAttackable),
        "AUTO_RALLYPOINT" | "AUTO_RALLY_POINT" => Some(KindOf::AutoRallypoint),
        "MONEY_HACKER" => Some(KindOf::MoneyHacker),
        "BALLISTIC_MISSILE" => Some(KindOf::BallisticMissile),
        "CLICK_THROUGH" => Some(KindOf::ClickThrough),
        "SHOW_PORTRAIT_WHEN_CONTROLLED" => Some(KindOf::ShowPortraitWhenControlled),
        "CANNOT_BUILD_NEAR_SUPPLIES" => Some(KindOf::CannotBuildNearSupplies),
        "REVEAL_TO_ALL" => Some(KindOf::RevealToAll),
        "IGNORES_SELECT_ALL" => Some(KindOf::IgnoresSelectAll),
        "DONT_AUTO_CRUSH_INFANTRY" => Some(KindOf::DontAutoCrushInfantry),
        "REVEALS_ENEMY_PATHS" => Some(KindOf::RevealsEnemyPaths),
        "NO_SELECT" => Some(KindOf::NoSelect),
        "CANNOT_RETALIATE" => Some(KindOf::CannotRetaliate),
        "TECH_BASE_DEFENSE" => Some(KindOf::TechBaseDefense),
        "DEMOTRAP" | "DEMO_TRAP" => Some(KindOf::Demotrap),
        "CONSERVATIVE_BUILDING" => Some(KindOf::ConservativeBuilding),
        "BLAST_CRATER" => Some(KindOf::BlastCrater),
        "PROP" => Some(KindOf::Prop),
        "OPTIMIZED_TREE" => Some(KindOf::OptimizedTree),
        "LANDMARK_BRIDGE" => Some(KindOf::LandmarkBridge),
        "WAVE_EFFECT" => Some(KindOf::WaveEffect),
        "CLEARED_BY_BUILD" => Some(KindOf::ClearedByBuild),
        "PARACHUTE" => Some(KindOf::Parachute),
        _ => None,
    }
}

