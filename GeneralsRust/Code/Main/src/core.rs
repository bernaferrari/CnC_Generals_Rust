use bitflags::bitflags;

// GameMode enum based on C++ GameLogic.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    SinglePlayer,
    Lan,
    Skirmish,
    Replay,
    Shell,
    Internet,
    None,
}

// DamageType enum based on C++ Damage.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageType {
    Explosion = 0,
    Crush = 1,
    ArmorPiercing = 2,
    SmallArms = 3,
    Gattling = 4,
    Radiation = 5,
    Flame = 6,
    Laser = 7,
    Sniper = 8,
    Poison = 9,
    Healing = 10,
    Unresistable = 11,
    Water = 12,
    Deploy = 13,
    Surrender = 14,
    Hack = 15,
    KillPilot = 16,
    Penalty = 17,
    Falling = 18,
    Melee = 19,
    Disarm = 20,
    HazardCleanup = 21,
    ParticleBeam = 22,
    Toppling = 23,
    InfantryMissile = 24,
    AuroraBomb = 25,
    LandMine = 26,
    JetMissiles = 27,
    StealthjetMissiles = 28,
    MolotovCocktail = 29,
    ComancheVulcan = 30,
    SubdualMissile = 31,
    SubdualVehicle = 32,
    SubdualBuilding = 33,
    SubdualUnresistable = 34,
    Microwave = 35,
    KillGarrisoned = 36,
    Status = 37,
}

// DeathType enum based on C++ Damage.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathType {
    Normal = 0,
    None = 1,
    Crushed = 2,
    Burned = 3,
    Exploded = 4,
    Poisoned = 5,
    Toppled = 6,
    Flooded = 7,
    Suicided = 8,
    Lasered = 9,
    Detonated = 10,
    Splatted = 11,
    PoisonedBeta = 12,
    Extra2 = 13,
    Extra3 = 14,
    Extra4 = 15,
    Extra5 = 16,
    Extra6 = 17,
    Extra7 = 18,
    Extra8 = 19,
    PoisonedGamma = 20,
}

// ObjectStatusTypes bitflags based on C++ ObjectStatusTypes.h
bitflags! {
    pub struct ObjectStatusMaskType: u32 {
        const NONE = 0;
        const DESTROYED = 1 << 0;
        const CAN_ATTACK = 1 << 1;
        const UNDER_CONSTRUCTION = 1 << 2;
        const UNSELECTABLE = 1 << 3;
        const NO_COLLISIONS = 1 << 4;
        const NO_ATTACK = 1 << 5;
        const AIRBORNE_TARGET = 1 << 6;
        const PARACHUTING = 1 << 7;
        const REPULSOR = 1 << 8;
        const HIJACKED = 1 << 9;
        const AFLAME = 1 << 10;
        const BURNED = 1 << 11;
        const WET = 1 << 12;
        const IS_FIRING_WEAPON = 1 << 13;
        const BRAKING = 1 << 14;
        const STEALTHED = 1 << 15;
        const DETECTED = 1 << 16;
        const CAN_STEALTH = 1 << 17;
        const SOLD = 1 << 18;
        const UNDERGOING_REPAIR = 1 << 19;
        const RECONSTRUCTING = 1 << 20;
        const MASKED = 1 << 21;
        const IS_ATTACKING = 1 << 22;
        const IS_USING_ABILITY = 1 << 23;
        const IS_AIMING_WEAPON = 1 << 24;
        const NO_ATTACK_FROM_AI = 1 << 25;
        const IGNORING_STEALTH = 1 << 26;
        const IS_CARBOMB = 1 << 27;
        const DECK_HEIGHT_OFFSET = 1 << 28;
        const RIDER1 = 1 << 29;
        const RIDER2 = 1 << 30;
        const RIDER3 = 1 << 31;
        // Note: More bits would require u64 or separate handling
    }
}

// KindOfType bitflags based on C++ KindOf.h
bitflags! {
    pub struct KindOfMaskType: u64 {
        const INVALID = 0;
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
        // Note: More bits would require additional handling
    }
}

// SpecialPowerType enum based on C++ SpecialPowerType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialPowerType {
    Invalid,
    // Superweapons
    DaisyCutter,
    ParadropAmerica,
    CarpetBomb,
    ClusterMines,
    EmpPulse,
    NapalmStrike,
    CashHack,
    NeutronMissile,
    SpySatellite,
    Defector,
    TerrorCell,
    Ambush,
    BlackMarketNuke,
    AnthraxBomb,
    ScudStorm,
    ArtilleryBarrage,
    A10ThunderboltStrike,
    DetonateDirtyNuke,
    // Special abilities
    MissileDefenderLaserGuidedMissiles,
    RemoteCharges,
    TimedCharges,
    HelixNapalmBomb,
    HackerDisableBuilding,
    TankhunterTntAttack,
    BlacklotusCaptureBuilding,
    BlacklotusDisableVehicleHack,
    BlacklotusStealCashHack,
    InfantryCaptureBuilding,
    RadarVanScan,
    SpyDrone,
    DisguiseAsVehicle,
    BoobyTrap,
    RepairVehicles,
    ParticleUplinkCannon,
    CashBounty,
    ChangeBattlePlans,
    CiaIntelligence,
    CleanupArea,
    LaunchBaikonurRocket,
    SpectreGunship,
    GpsScrambler,
    Frenzy,
    SneakAttack,
    // Additional variants as needed
}

// ScienceType enum based on C++ Science.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScienceType {
    Invalid = -1,
    // Add specific sciences as needed
}

// UpgradeType enum based on C++ Upgrade.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeType {
    Player = 0,
    Object = 1,
}

// ScriptActionType enum based on C++ Scripts.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptActionType {
    DebugMessageBox,
    SetFlag,
    SetCounter,
    Victory,
    Defeat,
    NoOp,
    SetTimer,
    PlaySoundEffect,
    EnableScript,
    DisableScript,
    CallSubroutine,
    PlaySoundEffectAt,
    DamageMembersOfTeam,
    MoveTeamTo,
    MoveCameraTo,
    IncrementCounter,
    DecrementCounter,
    MoveCameraAlongWaypointPath,
    RotateCamera,
    ResetCamera,
    SetMillisecondTimer,
    CameraModFreezeTime,
    SetVisualSpeedMultiplier,
    CreateObject,
    SuspendBackgroundSounds,
    ResumeBackgroundSounds,
    CameraModSetFinalZoom,
    CameraModSetFinalPitch,
    CameraModFreezeAngle,
    CameraModSetFinalSpeedMultiplier,
    CameraModSetRollingAverage,
    CameraModFinalLookToward,
    CameraModLookToward,
    TeamAttackTeam,
    CreateReinforcementTeam,
    MoveCameraToSelection,
    TeamFollowWaypoints,
    TeamSetState,
    MoveNamedUnitTo,
    NamedAttackNamed,
    CreateNamedOnTeamAtWaypoint,
    CreateUnnamedOnTeamAtWaypoint,
    NamedApplyAttackPrioritySet,
    TeamApplyAttackPrioritySet,
    SetBaseConstructionSpeed,
    NamedSetAttitude,
    TeamSetAttitude,
    NamedAttackArea,
    NamedAttackTeam,
    TeamAttackArea,
    TeamAttackNamed,
    TeamLoadTransports,
    NamedEnterNamed,
    TeamEnterNamed,
    NamedExitAll,
    TeamExitAll,
    NamedFollowWaypoints,
    NamedGuard,
    TeamGuard,
    NamedHunt,
    TeamHunt,
    PlayerSellEverything,
    PlayerDisableBaseConstruction,
    PlayerDisableFactories,
    PlayerDisableUnitConstruction,
    PlayerEnableBaseConstruction,
    PlayerEnableFactories,
    PlayerEnableUnitConstruction,
    CameraMoveHome,
    BuildTeam,
    NamedDamage,
    NamedDelete,
    TeamDelete,
    NamedKill,
    TeamKill,
    PlayerKill,
    DisplayText,
    CameoFlash,
    NamedFlash,
    TeamFlash,
    MoviePlayFullscreen,
    MoviePlayRadar,
    SoundPlayNamed,
    SpeechPlay,
    PlayerTransferOwnershipPlayer,
    NamedTransferOwnershipPlayer,
    PlayerRelatesPlayer,
    RadarCreateEvent,
    RadarDisable,
    RadarEnable,
    MapRevealAtWaypoint,
    TeamAvailableForRecruitment,
    TeamCollectNearbyForTeam,
    TeamMergeIntoTeam,
    DisableInput,
    EnableInput,
    PlayerHunt,
    SoundAmbientPause,
    SoundAmbientResume,
    MusicSetTrack,
    SetTreeSway,
    DebugString,
    MapRevealAll,
    TeamGarrisonSpecificBuilding,
    ExitSpecificBuilding,
    TeamGarrisonNearestBuilding,
    TeamExitAllBuildings,
    NamedGarrisonSpecificBuilding,
    NamedGarrisonNearestBuilding,
    NamedExitBuilding,
    PlayerGarrisonAllBuildings,
    PlayerExitAllBuildings,
    TeamWander,
    TeamPanic,
    SetupCamera,
    CameraLetterboxBegin,
    CameraLetterboxEnd,
    ZoomCamera,
    PitchCamera,
    CameraFollowNamed,
    OversizeTerrain,
    CameraFadeAdd,
    CameraFadeSubtract,
    CameraFadeSaturate,
    CameraFadeMultiply,
    CameraBwModeBegin,
    CameraBwModeEnd,
    DrawSkyboxBegin,
    DrawSkyboxEnd,
    SetAttackPriorityThing,
    SetAttackPriorityKindOf,
    SetDefaultAttackPriority,
    CameraStopFollow,
    CameraMotionBlur,
    CameraMotionBlurJump,
    CameraMotionBlurFollow,
    CameraMotionBlurEndFollow,
    FreezeTime,
    UnfreezeTime,
    ShowMilitaryCaption,
    CameraSetAudibleDistance,
    SetStoppingDistance,
    NamedSetStoppingDistance,
    SetFpsLimit,
    MusicSetVolume,
    MapShroudAtWaypoint,
    MapShroudAll,
    SetRandomTimer,
    SetRandomMsecTimer,
    StopTimer,
    RestartTimer,
    AddToMsecTimer,
    SubFromMsecTimer,
    TeamTransferToPlayer,
    PlayerSetMoney,
    PlayerGiveMoney,
    DisableSpecialPowerDisplay,
    EnableSpecialPowerDisplay,
    NamedDisableSpecialPowerDisplay,
    NamedEnableSpecialPowerDisplay,
    DisplayCountdownTimer,
    HideCountdownTimer,
    EnableCountdownTimerDisplay,
    DisableCountdownTimerDisplay,
    NamedStopSpecialPowerCountdown,
    NamedStartSpecialPowerCountdown,
    NamedSetSpecialPowerCountdown,
    NamedAddSpecialPowerCountdown,
    NamedFireSpecialPowerAtWaypoint,
    NamedFireSpecialPowerAtNamed,
    RefreshRadar,
    CameraTetherNamed,
    CameraStopTetherNamed,
    CameraSetDefault,
    NamedStop,
    TeamStop,
    TeamStopAndDisband,
    RecruitTeam,
    TeamSetOverrideRelationToTeam,
    TeamRemoveOverrideRelationToTeam,
    TeamRemoveAllOverrideRelations,
    CameraLookTowardObject,
    NamedFireWeaponFollowingWaypointPath,
    TeamSetOverrideRelationToPlayer,
    TeamRemoveOverrideRelationToPlayer,
    UnitExecuteSequentialScript,
    UnitExecuteSequentialScriptLooping,
    UnitStopSequentialScript,
    TeamExecuteSequentialScript,
    TeamExecuteSequentialScriptLooping,
    TeamStopSequentialScript,
    UnitGuardForFramecount,
    UnitIdleForFramecount,
    TeamGuardForFramecount,
    TeamIdleForFramecount,
    WaterChangeHeight,
    NamedUseCommandbuttonAbilityOnNamed,
    NamedUseCommandbuttonAbilityAtWaypoint,
    WaterChangeHeightOverTime,
    MapSwitchBorder,
    TeamGuardPosition,
    TeamGuardObject,
    TeamGuardArea,
    ObjectForceSelect,
    CameraLookTowardWaypoint,
    UnitDestroyAllContained,
    RadarForceEnable,
    RadarRevertToNormal,
    ScreenShake,
    TechtreeModifyBuildabilityObject,
    WarehouseSetValue,
    ObjectCreateRadarEvent,
    TeamCreateRadarEvent,
    DisplayCinematicText,
    DebugCrashBox,
    SoundDisableType,
    SoundEnableType,
    SoundEnableAll,
    AudioOverrideVolumeType,
    AudioRestoreVolumeType,
    AudioRestoreVolumeAllType,
    IngamePopupMessage,
    SetCaveIndex,
    NamedSetHeld,
    NamedSetToppleDirection,
    UnitMoveTowardsNearestObjectType,
    TeamMoveTowardsNearestObjectType,
    MapRevealAllPerm,
    MapUndoRevealAllPerm,
    NamedSetRepulsor,
    TeamSetRepulsor,
    TeamWanderInPlace,
    TeamIncreasePriority,
    TeamDecreasePriority,
    DisplayCounter,
    HideCounter,
    TeamUseCommandbuttonAbilityOnNamed,
    TeamUseCommandbuttonAbilityAtWaypoint,
    NamedUseCommandbuttonAbility,
    TeamUseCommandbuttonAbility,
    NamedFlashWhite,
    TeamFlashWhite,
    SkirmishBuildBuilding,
    SkirmishFollowApproachPath,
    IdleAllUnits,
    ResumeSupplyTrucking,
    NamedCustomColor,
    SkirmishMoveToApproachPath,
    SkirmishBuildBaseDefenseFront,
    SkirmishFireSpecialPowerAtMostCost,
    NamedReceiveUpgrade,
    PlayerRepairNamedStructure,
    SkirmishBuildBaseDefenseFlank,
    SkirmishBuildStructureFront,
    SkirmishBuildStructureFlank,
    SkirmishAttackNearestGroupWithValue,
    SkirmishPerformCommandbuttonOnMostValuableObject,
    SkirmishWaitForCommandbuttonAvailableAll,
    SkirmishWaitForCommandbuttonAvailablePartial,
    TeamSpinForFramecount,
    TeamAllUseCommandbuttonOnNamed,
    TeamAllUseCommandbuttonOnNearestEnemyUnit,
    TeamAllUseCommandbuttonOnNearestGarrisonedBuilding,
    TeamAllUseCommandbuttonOnNearestKindof,
    TeamAllUseCommandbuttonOnNearestEnemyBuilding,
    TeamAllUseCommandbuttonOnNearestEnemyBuildingClass,
    TeamAllUseCommandbuttonOnNearestObjecttype,
    TeamPartialUseCommandbutton,
    TeamCaptureNearestUnownedFactionUnit,
    PlayerCreateTeamFromCapturedUnits,
    PlayerAddSkillpoints,
    PlayerAddRanklevel,
    PlayerSetRanklevel,
    PlayerSetRanklevellimit,
    PlayerGrantScience,
    PlayerPurchaseScience,
    TeamHuntWithCommandButton,
    TeamWaitForNotContainedAll,
    TeamWaitForNotContainedPartial,
    TeamFollowWaypointsExact,
    NamedFollowWaypointsExact,
    TeamSetEmoticon,
    NamedSetEmoticon,
    AiPlayerBuildSupplyCenter,
    AiPlayerBuildUpgrade,
    ObjectlistAddobjecttype,
    ObjectlistRemoveobjecttype,
    MapRevealPermanentAtWaypoint,
    MapUndoRevealPermanentAtWaypoint,
    NamedSetStealthEnabled,
    TeamSetStealthEnabled,
    EvaSetEnabledDisabled,
    OptionsSetOcclusionMode,
    OptionsSetDrawiconUiMode,
    OptionsSetParticleCapMode,
    PlayerScienceAvailability,
    UnitAffectObjectPanelFlags,
    TeamAffectObjectPanelFlags,
    PlayerSelectSkillset,
    ScriptingOverrideHulkLifetime,
    NamedFaceNamed,
    NamedFaceWaypoint,
    TeamFaceNamed,
    TeamFaceWaypoint,
    CommandbarRemoveButtonObjecttype,
    CommandbarAddButtonObjecttypeSlot,
    UnitSpawnNamedLocationOrientation,
    PlayerAffectReceivingExperience,
    PlayerExcludeFromScoreScreen,
    TeamGuardSupplyCenter,
    EnableScoring,
    DisableScoring,
    SoundSetVolume,
    SpeechSetVolume,
    DisableBorderShroud,
    EnableBorderShroud,
    ObjectAllowBonuses,
    SoundRemoveAllDisabled,
    SoundRemoveType,
    TeamGuardInTunnelNetwork,
    Quickvictory,
    SetInfantryLightingOverride,
    ResetInfantryLightingOverride,
    TeamDeleteLiving,
    ResizeViewGuardband,
    DeleteAllUnmanned,
    ChooseVictimAlwaysUsesNormal,
    CameraEnableSlaveMode,
    CameraDisableSlaveMode,
    CameraAddShakerAt,
    SetTrainHeld,
    NamedSetEvacLeftOrRight,
    EnableObjectSound,
    DisableObjectSound,
    NamedUseCommandbuttonAbilityUsingWaypointPath,
    NamedSetUnmannedStatus,
    TeamSetUnmannedStatus,
    NamedSetBoobytrapped,
    TeamSetBoobytrapped,
    ShowWeather,
    AiPlayerBuildTypeNearestTeam,
}

// ConditionType enum based on C++ Scripts.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionType {
    False,
    Counter,
    Flag,
    True,
    TimerExpired,
    PlayerAllDestroyed,
    PlayerAllBuildfacilitiesDestroyed,
    TeamInsideAreaPartially,
    TeamDestroyed,
    CameraMovementFinished,
    TeamHasUnits,
    TeamStateIs,
    TeamStateIsNot,
    NamedInsideArea,
    NamedOutsideArea,
    NamedDestroyed,
    NamedNotDestroyed,
    TeamInsideAreaEntirely,
    TeamOutsideAreaEntirely,
    NamedAttackedByObjecttype,
    TeamAttackedByObjecttype,
    NamedAttackedByPlayer,
    TeamAttackedByPlayer,
    BuiltByPlayer,
    NamedCreated,
    TeamCreated,
    PlayerHasCredits,
    NamedDiscovered,
    TeamDiscovered,
    MissionAttempts,
    NamedOwnedByPlayer,
    TeamOwnedByPlayer,
    PlayerHasNOrFewerBuildings,
    PlayerHasPower,
    NamedReachedWaypointsEnd,
    TeamReachedWaypointsEnd,
    NamedSelected,
    NamedEnteredArea,
    NamedExitedArea,
    TeamEnteredAreaEntirely,
    TeamEnteredAreaPartially,
    TeamExitedAreaEntirely,
    TeamExitedAreaPartially,
    MultiplayerAlliedVictory,
    MultiplayerAlliedDefeat,
    MultiplayerPlayerDefeat,
    PlayerHasNoPower,
    HasFinishedVideo,
    HasFinishedSpeech,
    HasFinishedAudio,
    BuildingEnteredByPlayer,
    EnemySighted,
    UnitHealth,
    BridgeRepaired,
    BridgeBroken,
    NamedDying,
    NamedTotallyDead,
    PlayerHasComparisonUnitTypeInTriggerArea,
    PlayerHasComparisonUnitKindInTriggerArea,
    UnitEmptied,
    TypeSighted,
    NamedBuildingIsEmpty,
    PlayerHasNOrFewerFactionBuildings,
    UnitHasObjectStatus,
    TeamAllHasObjectStatus,
    TeamSomeHaveObjectStatus,
    PlayerPowerComparePercent,
    PlayerExcessPowerCompareValue,
    SkirmishSpecialPowerReady,
    SkirmishValueInArea,
    SkirmishPlayerFaction,
    SkirmishSuppliesValueWithinDistance,
    SkirmishTechBuildingWithinDistance,
    SkirmishCommandButtonReadyAll,
    SkirmishCommandButtonReadyPartial,
    SkirmishUnownedFactionUnitExists,
    SkirmishPlayerHasPrerequisiteToBuild,
    SkirmishPlayerHasComparisonGarrisoned,
    SkirmishPlayerHasComparisonCapturedUnits,
    SkirmishNamedAreaExist,
    PlayerAcquiredScience,
    PlayerHasSciencepurchasepoints,
    PlayerCanPurchaseScience,
    MusicTrackHasCompleted,
    PlayerLostObjectType,
    SupplySourceSafe,
    SupplySourceAttacked,
    StartPositionIs,
    NamedHasFreeContainerSlots,
}

// UpdateSleepTime enum based on C++ UpdateModule.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSleepTime {
    Invalid = 0,
    None = 1,
    Forever = 0x3fffffff,
}

// Coord3D struct based on C++ BaseType.h
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn length_sqr(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn normalize(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    pub fn add(&mut self, other: &Coord3D) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }

    pub fn sub(&mut self, other: &Coord3D) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }

    pub fn set(&mut self, other: &Coord3D) {
        self.x = other.x;
        self.y = other.y;
        self.z = other.z;
    }

    pub fn scale(&mut self, scale: f32) {
        self.x *= scale;
        self.y *= scale;
        self.z *= scale;
    }
}

// DamageInfo struct based on C++ Damage.h
#[derive(Debug, Clone)]
pub struct DamageInfo {
    pub source_id: u32,
    pub damage_type: DamageType,
    pub death_type: DeathType,
    pub amount: f32,
    pub kill: bool,
}

impl DamageInfo {
    pub fn new(damage_type: DamageType, death_type: DeathType, amount: f32) -> Self {
        Self {
            source_id: 0,
            damage_type,
            death_type,
            amount,
            kill: false,
        }
    }
}