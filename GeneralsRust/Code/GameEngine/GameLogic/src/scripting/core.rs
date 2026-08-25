//! Core Script System Classes
//!
//! This module provides the core script system classes that mirror the C++ implementation,
//! including Script, ScriptAction, Condition, Parameter, and Template classes.

use crate::{GameLogicError, GameLogicResult};
use serde::{Deserialize, Serialize};

use crate::common::ALL_KIND_OF;
use crate::common::{KindOf, ObjectStatusMaskType};
use crate::object::behavior::auto_heal_behavior::parse_kind_of;
use crate::scripting::XferSnapshot;
use crate::scripting::engine::get_script_engine;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use game_engine::common::system::{Xfer, XferStatus, XferVersion};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

pub const MAX_PARMS: usize = 12;
pub const THIS_TEAM: &str = "<This Team>";
pub const ANY_TEAM: &str = "<Any Team>";
pub const THIS_OBJECT: &str = "<This Object>";
pub const ANY_OBJECT: &str = "<Any Object>";
pub const THIS_PLAYER: &str = "<This Player>";
pub const LOCAL_PLAYER: &str = "<Local Player>";
pub const THE_PLAYER: &str = "ThePlayer";
pub const TEAM_THE_PLAYER: &str = "teamThePlayer";
pub const THIS_PLAYER_ENEMY: &str = "<This Player's Enemy>";

/// C++ ScriptEngine.cpp:5810 / 5935 — THE_PLAYER and teamThePlayer are
/// Challenge-only aliases. Regular single-player maps author a real side
/// named ThePlayer that must not be remapped to the local general.
pub fn is_generals_challenge_campaign() -> bool {
    game_engine::System::capture_campaign_manager_runtime().is_challenge
}

const K_SCRIPTS_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_LIST_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_GROUP_DATA_VERSION_2: u16 = 2;
const K_SCRIPT_DATA_VERSION_2: u16 = 2;
const K_SCRIPT_ACTION_VERSION_2: u16 = 2;
const K_SCRIPT_CONDITION_VERSION_4: u16 = 4;
const K_SCRIPT_OR_CONDITION_DATA_VERSION_1: u16 = 1;
static SCRIPT_GROUP_ID: AtomicU32 = AtomicU32::new(0);

/// Coordinate 3D structure matching C++
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Script action type enumeration matching the C++ ScriptActionType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ScriptActionType {
    DebugMessageBox = 0,
    SetFlag = 1,
    SetCounter = 2,
    Victory = 3,
    Defeat = 4,
    NoOp = 5,
    SetTimer = 6,
    PlaySoundEffect = 7,
    EnableScript = 8,
    DisableScript = 9,
    CallSubroutine = 10,
    PlaySoundEffectAt = 11,
    DamageMembersOfTeam = 12,
    MoveTeamTo = 13,
    MoveCameraTo = 14,
    IncrementCounter = 15,
    DecrementCounter = 16,
    MoveCameraAlongWaypointPath = 17,
    RotateCamera = 18,
    ResetCamera = 19,
    SetMillisecondTimer = 20,
    CameraModFreezeTime = 21,
    SetVisualSpeedMultiplier = 22,
    CreateObject = 23,
    SuspendBackgroundSounds = 24,
    ResumeBackgroundSounds = 25,
    CameraModSetFinalZoom = 26,
    CameraModSetFinalPitch = 27,
    CameraModFreezeAngle = 28,
    CameraModSetFinalSpeedMultiplier = 29,
    CameraModSetRollingAverage = 30,
    CameraModFinalLookToward = 31,
    CameraModLookToward = 32,
    TeamAttackTeam = 33,
    CreateReinforcementTeam = 34,
    MoveCameraToSelection = 35,
    TeamFollowWaypoints = 36,
    TeamSetState = 37,
    MoveNamedUnitTo = 38,
    NamedAttackNamed = 39,
    CreateNamedOnTeamAtWaypoint = 40,
    CreateUnnamedOnTeamAtWaypoint = 41,
    NamedApplyAttackPrioritySet = 42,
    TeamApplyAttackPrioritySet = 43,
    SetBaseConstructionSpeed = 44,
    NamedSetAttitude = 45,
    TeamSetAttitude = 46,
    NamedAttackArea = 47,
    NamedAttackTeam = 48,
    TeamAttackArea = 49,
    TeamAttackNamed = 50,
    TeamLoadTransports = 51,
    NamedEnterNamed = 52,
    TeamEnterNamed = 53,
    NamedExitAll = 54,
    TeamExitAll = 55,
    NamedFollowWaypoints = 56,
    NamedGuard = 57,
    TeamGuard = 58,
    NamedHunt = 59,
    TeamHunt = 60,
    PlayerSellEverything = 61,
    PlayerDisableBaseConstruction = 62,
    PlayerDisableFactories = 63,
    PlayerDisableUnitConstruction = 64,
    PlayerEnableBaseConstruction = 65,
    PlayerEnableFactories = 66,
    PlayerEnableUnitConstruction = 67,
    CameraMoveHome = 68,
    BuildTeam = 69,
    NamedDamage = 70,
    NamedDelete = 71,
    TeamDelete = 72,
    NamedKill = 73,
    TeamKill = 74,
    PlayerKill = 75,
    DisplayText = 76,
    CameoFlash = 77,
    NamedFlash = 78,
    TeamFlash = 79,
    MoviePlayFullscreen = 80,
    MoviePlayRadar = 81,
    SoundPlayNamed = 82,
    SpeechPlay = 83,
    PlayerTransferOwnershipPlayer = 84,
    NamedTransferOwnershipPlayer = 85,
    PlayerRelatesPlayer = 86,
    RadarCreateEvent = 87,
    RadarDisable = 88,
    RadarEnable = 89,
    MapRevealAtWaypoint = 90,
    TeamAvailableForRecruitment = 91,
    TeamCollectNearbyForTeam = 92,
    TeamMergeIntoTeam = 93,
    DisableInput = 94,
    EnableInput = 95,
    PlayerHunt = 96,
    SoundAmbientPause = 97,
    SoundAmbientResume = 98,
    MusicSetTrack = 99,
    SetTreeSway = 100,
    DebugString = 101,
    MapRevealAll = 102,
    TeamGarrisonSpecificBuilding = 103,
    ExitSpecificBuilding = 104,
    TeamGarrisonNearestBuilding = 105,
    TeamExitAllBuildings = 106,
    NamedGarrisonSpecificBuilding = 107,
    NamedGarrisonNearestBuilding = 108,
    NamedExitBuilding = 109,
    PlayerGarrisonAllBuildings = 110,
    PlayerExitAllBuildings = 111,
    TeamWander = 112,
    TeamPanic = 113,
    SetupCamera = 114,
    CameraLetterboxBegin = 115,
    CameraLetterboxEnd = 116,
    ZoomCamera = 117,
    PitchCamera = 118,
    CameraFollowNamed = 119,
    OversizeTerrain = 120,
    CameraFadeAdd = 121,
    CameraFadeSubtract = 122,
    CameraFadeSaturate = 123,
    CameraFadeMultiply = 124,
    CameraBwModeBegin = 125,
    CameraBwModeEnd = 126,
    DrawSkyboxBegin = 127,
    DrawSkyboxEnd = 128,
    SetAttackPriorityThing = 129,
    SetAttackPriorityKindOf = 130,
    SetDefaultAttackPriority = 131,
    CameraStopFollow = 132,
    CameraMotionBlur = 133,
    CameraMotionBlurJump = 134,
    CameraMotionBlurFollow = 135,
    CameraMotionBlurEndFollow = 136,
    FreezeTime = 137,
    UnfreezeTime = 138,
    ShowMilitaryCaption = 139,
    CameraSetAudibleDistance = 140,
    SetStoppingDistance = 141,
    NamedSetStoppingDistance = 142,
    SetFpsLimit = 143,
    MusicSetVolume = 144,
    MapShroudAtWaypoint = 145,
    MapShroudAll = 146,
    SetRandomTimer = 147,
    SetRandomMsecTimer = 148,
    StopTimer = 149,
    RestartTimer = 150,
    AddToMsecTimer = 151,
    SubFromMsecTimer = 152,
    TeamTransferToPlayer = 153,
    PlayerSetMoney = 154,
    PlayerGiveMoney = 155,
    DisableSpecialPowerDisplay = 156,
    EnableSpecialPowerDisplay = 157,
    NamedHideSpecialPowerDisplay = 158,
    NamedShowSpecialPowerDisplay = 159,
    DisplayCountdownTimer = 160,
    HideCountdownTimer = 161,
    EnableCountdownTimerDisplay = 162,
    DisableCountdownTimerDisplay = 163,
    NamedStopSpecialPowerCountdown = 164,
    NamedStartSpecialPowerCountdown = 165,
    NamedSetSpecialPowerCountdown = 166,
    NamedAddSpecialPowerCountdown = 167,
    NamedFireSpecialPowerAtWaypoint = 168,
    NamedFireSpecialPowerAtNamed = 169,
    RefreshRadar = 170,
    CameraTetherNamed = 171,
    CameraStopTetherNamed = 172,
    CameraSetDefault = 173,
    NamedStop = 174,
    TeamStop = 175,
    TeamStopAndDisband = 176,
    RecruitTeam = 177,
    TeamSetOverrideRelationToTeam = 178,
    TeamRemoveOverrideRelationToTeam = 179,
    TeamRemoveAllOverrideRelations = 180,
    CameraLookTowardObject = 181,
    NamedFireWeaponFollowingWaypointPath = 182,
    TeamSetOverrideRelationToPlayer = 183,
    TeamRemoveOverrideRelationToPlayer = 184,
    PlayerSetOverrideRelationToTeam = 185,
    PlayerRemoveOverrideRelationToTeam = 186,
    UnitExecuteSequentialScript = 187,
    UnitExecuteSequentialScriptLooping = 188,
    UnitStopSequentialScript = 189,
    TeamExecuteSequentialScript = 190,
    TeamExecuteSequentialScriptLooping = 191,
    TeamStopSequentialScript = 192,
    UnitGuardForFramecount = 193,
    UnitIdleForFramecount = 194,
    TeamGuardForFramecount = 195,
    TeamIdleForFramecount = 196,
    WaterChangeHeight = 197,
    NamedUseCommandbuttonAbilityOnNamed = 198,
    NamedUseCommandbuttonAbilityAtWaypoint = 199,
    WaterChangeHeightOverTime = 200,
    MapSwitchBorder = 201,
    TeamGuardPosition = 202,
    TeamGuardObject = 203,
    TeamGuardArea = 204,
    ObjectForceSelect = 205,
    CameraLookTowardWaypoint = 206,
    UnitDestroyAllContained = 207,
    RadarForceEnable = 208,
    RadarRevertToNormal = 209,
    ScreenShake = 210,
    TechtreeModifyBuildabilityObject = 211,
    WarehouseSetValue = 212,
    ObjectCreateRadarEvent = 213,
    TeamCreateRadarEvent = 214,
    DisplayCinematicText = 215,
    DebugCrashBox = 216,
    SoundDisableType = 217,
    SoundEnableType = 218,
    SoundEnableAll = 219,
    AudioOverrideVolumeType = 220,
    AudioRestoreVolumeType = 221,
    AudioRestoreVolumeAllType = 222,
    IngamePopupMessage = 223,
    SetCaveIndex = 224,
    NamedSetHeld = 225,
    NamedSetToppleDirection = 226,
    UnitMoveTowardsNearestObjectType = 227,
    TeamMoveTowardsNearestObjectType = 228,
    MapRevealAllPerm = 229,
    MapRevealAllUndoPerm = 230,
    NamedSetRepulsor = 231,
    TeamSetRepulsor = 232,
    TeamWanderInPlace = 233,
    TeamIncreasePriority = 234,
    TeamDecreasePriority = 235,
    DisplayCounter = 236,
    HideCounter = 237,
    TeamUseCommandbuttonAbilityOnNamed = 238,
    TeamUseCommandbuttonAbilityAtWaypoint = 239,
    NamedUseCommandbuttonAbility = 240,
    TeamUseCommandbuttonAbility = 241,
    NamedFlashWhite = 242,
    TeamFlashWhite = 243,
    SkirmishBuildBuilding = 244,
    SkirmishFollowApproachPath = 245,
    IdleAllUnits = 246,
    ResumeSupplyTrucking = 247,
    NamedCustomColor = 248,
    SkirmishMoveToApproachPath = 249,
    SkirmishBuildBaseDefenseFront = 250,
    SkirmishFireSpecialPowerAtMostCost = 251,
    NamedReceiveUpgrade = 252,
    PlayerRepairNamedStructure = 253,
    SkirmishBuildBaseDefenseFlank = 254,
    SkirmishBuildStructureFront = 255,
    SkirmishBuildStructureFlank = 256,
    SkirmishAttackNearestGroupWithValue = 257,
    SkirmishPerformCommandbuttonOnMostValuableObject = 258,
    SkirmishWaitForCommandbuttonAvailableAll = 259,
    SkirmishWaitForCommandbuttonAvailablePartial = 260,
    TeamSpinForFramecount = 261,
    TeamAllUseCommandbuttonOnNamed = 262,
    TeamAllUseCommandbuttonOnNearestEnemyUnit = 263,
    TeamAllUseCommandbuttonOnNearestGarrisonedBuilding = 264,
    TeamAllUseCommandbuttonOnNearestKindof = 265,
    TeamAllUseCommandbuttonOnNearestEnemyBuilding = 266,
    TeamAllUseCommandbuttonOnNearestEnemyBuildingClass = 267,
    TeamAllUseCommandbuttonOnNearestObjecttype = 268,
    TeamPartialUseCommandbutton = 269,
    TeamCaptureNearestUnownedFactionUnit = 270,
    PlayerCreateTeamFromCapturedUnits = 271,
    PlayerAddSkillpoints = 272,
    PlayerAddRanklevel = 273,
    PlayerSetRanklevel = 274,
    PlayerSetRanklevellimit = 275,
    PlayerGrantScience = 276,
    PlayerPurchaseScience = 277,
    TeamHuntWithCommandButton = 278,
    TeamWaitForNotContainedAll = 279,
    TeamWaitForNotContainedPartial = 280,
    TeamFollowWaypointsExact = 281,
    NamedFollowWaypointsExact = 282,
    TeamSetEmoticon = 283,
    NamedSetEmoticon = 284,
    AiPlayerBuildSupplyCenter = 285,
    AiPlayerBuildUpgrade = 286,
    ObjectlistAddobjecttype = 287,
    ObjectlistRemoveobjecttype = 288,
    MapRevealPermanentlyAtWaypoint = 289,
    MapUndoRevealPermanentlyAtWaypoint = 290,
    NamedSetStealthEnabled = 291,
    TeamSetStealthEnabled = 292,
    EvaSetEnabledDisabled = 293,
    OptionsSetOcclusionMode = 294,
    Localdefeat = 295,
    OptionsSetDrawiconUiMode = 296,
    OptionsSetParticleCapMode = 297,
    PlayerScienceAvailability = 298,
    UnitAffectObjectPanelFlags = 299,
    TeamAffectObjectPanelFlags = 300,
    PlayerSelectSkillset = 301,
    ScriptingOverrideHulkLifetime = 302,
    NamedFaceNamed = 303,
    NamedFaceWaypoint = 304,
    TeamFaceNamed = 305,
    TeamFaceWaypoint = 306,
    CommandbarRemoveButtonObjecttype = 307,
    CommandbarAddButtonObjecttypeSlot = 308,
    UnitSpawnNamedLocationOrientation = 309,
    PlayerAffectReceivingExperience = 310,
    PlayerExcludeFromScoreScreen = 311,
    TeamGuardSupplyCenter = 312,
    EnableScoring = 313,
    DisableScoring = 314,
    SoundSetVolume = 315,
    SpeechSetVolume = 316,
    DisableBorderShroud = 317,
    EnableBorderShroud = 318,
    ObjectAllowBonuses = 319,
    SoundRemoveAllDisabled = 320,
    SoundRemoveType = 321,
    TeamGuardInTunnelNetwork = 322,
    Quickvictory = 323,
    SetInfantryLightingOverride = 324,
    ResetInfantryLightingOverride = 325,
    TeamDeleteLiving = 326,
    ResizeViewGuardband = 327,
    DeleteAllUnmanned = 328,
    ChooseVictimAlwaysUsesNormal = 329,
    CameraEnableSlaveMode = 330,
    CameraDisableSlaveMode = 331,
    CameraAddShakerAt = 332,
    SetTrainHeld = 333,
    NamedSetEvacLeftOrRight = 334,
    EnableObjectSound = 335,
    DisableObjectSound = 336,
    NamedUseCommandbuttonAbilityUsingWaypointPath = 337,
    NamedSetUnmannedStatus = 338,
    TeamSetUnmannedStatus = 339,
    NamedSetBoobytrapped = 340,
    TeamSetBoobytrapped = 341,
    ShowWeather = 342,
    AiPlayerBuildTypeNearestTeam = 343,
    NumItems = 344,
}

impl ScriptActionType {
    pub fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::DebugMessageBox,
            1 => Self::SetFlag,
            2 => Self::SetCounter,
            3 => Self::Victory,
            4 => Self::Defeat,
            5 => Self::NoOp,
            6 => Self::SetTimer,
            7 => Self::PlaySoundEffect,
            8 => Self::EnableScript,
            9 => Self::DisableScript,
            10 => Self::CallSubroutine,
            11 => Self::PlaySoundEffectAt,
            12 => Self::DamageMembersOfTeam,
            13 => Self::MoveTeamTo,
            14 => Self::MoveCameraTo,
            15 => Self::IncrementCounter,
            16 => Self::DecrementCounter,
            17 => Self::MoveCameraAlongWaypointPath,
            18 => Self::RotateCamera,
            19 => Self::ResetCamera,
            20 => Self::SetMillisecondTimer,
            21 => Self::CameraModFreezeTime,
            22 => Self::SetVisualSpeedMultiplier,
            23 => Self::CreateObject,
            24 => Self::SuspendBackgroundSounds,
            25 => Self::ResumeBackgroundSounds,
            26 => Self::CameraModSetFinalZoom,
            27 => Self::CameraModSetFinalPitch,
            28 => Self::CameraModFreezeAngle,
            29 => Self::CameraModSetFinalSpeedMultiplier,
            30 => Self::CameraModSetRollingAverage,
            31 => Self::CameraModFinalLookToward,
            32 => Self::CameraModLookToward,
            33 => Self::TeamAttackTeam,
            34 => Self::CreateReinforcementTeam,
            35 => Self::MoveCameraToSelection,
            36 => Self::TeamFollowWaypoints,
            37 => Self::TeamSetState,
            38 => Self::MoveNamedUnitTo,
            39 => Self::NamedAttackNamed,
            40 => Self::CreateNamedOnTeamAtWaypoint,
            41 => Self::CreateUnnamedOnTeamAtWaypoint,
            42 => Self::NamedApplyAttackPrioritySet,
            43 => Self::TeamApplyAttackPrioritySet,
            44 => Self::SetBaseConstructionSpeed,
            45 => Self::NamedSetAttitude,
            46 => Self::TeamSetAttitude,
            47 => Self::NamedAttackArea,
            48 => Self::NamedAttackTeam,
            49 => Self::TeamAttackArea,
            50 => Self::TeamAttackNamed,
            51 => Self::TeamLoadTransports,
            52 => Self::NamedEnterNamed,
            53 => Self::TeamEnterNamed,
            54 => Self::NamedExitAll,
            55 => Self::TeamExitAll,
            56 => Self::NamedFollowWaypoints,
            57 => Self::NamedGuard,
            58 => Self::TeamGuard,
            59 => Self::NamedHunt,
            60 => Self::TeamHunt,
            61 => Self::PlayerSellEverything,
            62 => Self::PlayerDisableBaseConstruction,
            63 => Self::PlayerDisableFactories,
            64 => Self::PlayerDisableUnitConstruction,
            65 => Self::PlayerEnableBaseConstruction,
            66 => Self::PlayerEnableFactories,
            67 => Self::PlayerEnableUnitConstruction,
            68 => Self::CameraMoveHome,
            69 => Self::BuildTeam,
            70 => Self::NamedDamage,
            71 => Self::NamedDelete,
            72 => Self::TeamDelete,
            73 => Self::NamedKill,
            74 => Self::TeamKill,
            75 => Self::PlayerKill,
            76 => Self::DisplayText,
            77 => Self::CameoFlash,
            78 => Self::NamedFlash,
            79 => Self::TeamFlash,
            80 => Self::MoviePlayFullscreen,
            81 => Self::MoviePlayRadar,
            82 => Self::SoundPlayNamed,
            83 => Self::SpeechPlay,
            84 => Self::PlayerTransferOwnershipPlayer,
            85 => Self::NamedTransferOwnershipPlayer,
            86 => Self::PlayerRelatesPlayer,
            87 => Self::RadarCreateEvent,
            88 => Self::RadarDisable,
            89 => Self::RadarEnable,
            90 => Self::MapRevealAtWaypoint,
            91 => Self::TeamAvailableForRecruitment,
            92 => Self::TeamCollectNearbyForTeam,
            93 => Self::TeamMergeIntoTeam,
            94 => Self::DisableInput,
            95 => Self::EnableInput,
            96 => Self::PlayerHunt,
            97 => Self::SoundAmbientPause,
            98 => Self::SoundAmbientResume,
            99 => Self::MusicSetTrack,
            100 => Self::SetTreeSway,
            101 => Self::DebugString,
            102 => Self::MapRevealAll,
            103 => Self::TeamGarrisonSpecificBuilding,
            104 => Self::ExitSpecificBuilding,
            105 => Self::TeamGarrisonNearestBuilding,
            106 => Self::TeamExitAllBuildings,
            107 => Self::NamedGarrisonSpecificBuilding,
            108 => Self::NamedGarrisonNearestBuilding,
            109 => Self::NamedExitBuilding,
            110 => Self::PlayerGarrisonAllBuildings,
            111 => Self::PlayerExitAllBuildings,
            112 => Self::TeamWander,
            113 => Self::TeamPanic,
            114 => Self::SetupCamera,
            115 => Self::CameraLetterboxBegin,
            116 => Self::CameraLetterboxEnd,
            117 => Self::ZoomCamera,
            118 => Self::PitchCamera,
            119 => Self::CameraFollowNamed,
            120 => Self::OversizeTerrain,
            121 => Self::CameraFadeAdd,
            122 => Self::CameraFadeSubtract,
            123 => Self::CameraFadeSaturate,
            124 => Self::CameraFadeMultiply,
            125 => Self::CameraBwModeBegin,
            126 => Self::CameraBwModeEnd,
            127 => Self::DrawSkyboxBegin,
            128 => Self::DrawSkyboxEnd,
            129 => Self::SetAttackPriorityThing,
            130 => Self::SetAttackPriorityKindOf,
            131 => Self::SetDefaultAttackPriority,
            132 => Self::CameraStopFollow,
            133 => Self::CameraMotionBlur,
            134 => Self::CameraMotionBlurJump,
            135 => Self::CameraMotionBlurFollow,
            136 => Self::CameraMotionBlurEndFollow,
            137 => Self::FreezeTime,
            138 => Self::UnfreezeTime,
            139 => Self::ShowMilitaryCaption,
            140 => Self::CameraSetAudibleDistance,
            141 => Self::SetStoppingDistance,
            142 => Self::NamedSetStoppingDistance,
            143 => Self::SetFpsLimit,
            144 => Self::MusicSetVolume,
            145 => Self::MapShroudAtWaypoint,
            146 => Self::MapShroudAll,
            147 => Self::SetRandomTimer,
            148 => Self::SetRandomMsecTimer,
            149 => Self::StopTimer,
            150 => Self::RestartTimer,
            151 => Self::AddToMsecTimer,
            152 => Self::SubFromMsecTimer,
            153 => Self::TeamTransferToPlayer,
            154 => Self::PlayerSetMoney,
            155 => Self::PlayerGiveMoney,
            156 => Self::DisableSpecialPowerDisplay,
            157 => Self::EnableSpecialPowerDisplay,
            158 => Self::NamedHideSpecialPowerDisplay,
            159 => Self::NamedShowSpecialPowerDisplay,
            160 => Self::DisplayCountdownTimer,
            161 => Self::HideCountdownTimer,
            162 => Self::EnableCountdownTimerDisplay,
            163 => Self::DisableCountdownTimerDisplay,
            164 => Self::NamedStopSpecialPowerCountdown,
            165 => Self::NamedStartSpecialPowerCountdown,
            166 => Self::NamedSetSpecialPowerCountdown,
            167 => Self::NamedAddSpecialPowerCountdown,
            168 => Self::NamedFireSpecialPowerAtWaypoint,
            169 => Self::NamedFireSpecialPowerAtNamed,
            170 => Self::RefreshRadar,
            171 => Self::CameraTetherNamed,
            172 => Self::CameraStopTetherNamed,
            173 => Self::CameraSetDefault,
            174 => Self::NamedStop,
            175 => Self::TeamStop,
            176 => Self::TeamStopAndDisband,
            177 => Self::RecruitTeam,
            178 => Self::TeamSetOverrideRelationToTeam,
            179 => Self::TeamRemoveOverrideRelationToTeam,
            180 => Self::TeamRemoveAllOverrideRelations,
            181 => Self::CameraLookTowardObject,
            182 => Self::NamedFireWeaponFollowingWaypointPath,
            183 => Self::TeamSetOverrideRelationToPlayer,
            184 => Self::TeamRemoveOverrideRelationToPlayer,
            185 => Self::PlayerSetOverrideRelationToTeam,
            186 => Self::PlayerRemoveOverrideRelationToTeam,
            187 => Self::UnitExecuteSequentialScript,
            188 => Self::UnitExecuteSequentialScriptLooping,
            189 => Self::UnitStopSequentialScript,
            190 => Self::TeamExecuteSequentialScript,
            191 => Self::TeamExecuteSequentialScriptLooping,
            192 => Self::TeamStopSequentialScript,
            193 => Self::UnitGuardForFramecount,
            194 => Self::UnitIdleForFramecount,
            195 => Self::TeamGuardForFramecount,
            196 => Self::TeamIdleForFramecount,
            197 => Self::WaterChangeHeight,
            198 => Self::NamedUseCommandbuttonAbilityOnNamed,
            199 => Self::NamedUseCommandbuttonAbilityAtWaypoint,
            200 => Self::WaterChangeHeightOverTime,
            201 => Self::MapSwitchBorder,
            202 => Self::TeamGuardPosition,
            203 => Self::TeamGuardObject,
            204 => Self::TeamGuardArea,
            205 => Self::ObjectForceSelect,
            206 => Self::CameraLookTowardWaypoint,
            207 => Self::UnitDestroyAllContained,
            208 => Self::RadarForceEnable,
            209 => Self::RadarRevertToNormal,
            210 => Self::ScreenShake,
            211 => Self::TechtreeModifyBuildabilityObject,
            212 => Self::WarehouseSetValue,
            213 => Self::ObjectCreateRadarEvent,
            214 => Self::TeamCreateRadarEvent,
            215 => Self::DisplayCinematicText,
            216 => Self::DebugCrashBox,
            217 => Self::SoundDisableType,
            218 => Self::SoundEnableType,
            219 => Self::SoundEnableAll,
            220 => Self::AudioOverrideVolumeType,
            221 => Self::AudioRestoreVolumeType,
            222 => Self::AudioRestoreVolumeAllType,
            223 => Self::IngamePopupMessage,
            224 => Self::SetCaveIndex,
            225 => Self::NamedSetHeld,
            226 => Self::NamedSetToppleDirection,
            227 => Self::UnitMoveTowardsNearestObjectType,
            228 => Self::TeamMoveTowardsNearestObjectType,
            229 => Self::MapRevealAllPerm,
            230 => Self::MapRevealAllUndoPerm,
            231 => Self::NamedSetRepulsor,
            232 => Self::TeamSetRepulsor,
            233 => Self::TeamWanderInPlace,
            234 => Self::TeamIncreasePriority,
            235 => Self::TeamDecreasePriority,
            236 => Self::DisplayCounter,
            237 => Self::HideCounter,
            238 => Self::TeamUseCommandbuttonAbilityOnNamed,
            239 => Self::TeamUseCommandbuttonAbilityAtWaypoint,
            240 => Self::NamedUseCommandbuttonAbility,
            241 => Self::TeamUseCommandbuttonAbility,
            242 => Self::NamedFlashWhite,
            243 => Self::TeamFlashWhite,
            244 => Self::SkirmishBuildBuilding,
            245 => Self::SkirmishFollowApproachPath,
            246 => Self::IdleAllUnits,
            247 => Self::ResumeSupplyTrucking,
            248 => Self::NamedCustomColor,
            249 => Self::SkirmishMoveToApproachPath,
            250 => Self::SkirmishBuildBaseDefenseFront,
            251 => Self::SkirmishFireSpecialPowerAtMostCost,
            252 => Self::NamedReceiveUpgrade,
            253 => Self::PlayerRepairNamedStructure,
            254 => Self::SkirmishBuildBaseDefenseFlank,
            255 => Self::SkirmishBuildStructureFront,
            256 => Self::SkirmishBuildStructureFlank,
            257 => Self::SkirmishAttackNearestGroupWithValue,
            258 => Self::SkirmishPerformCommandbuttonOnMostValuableObject,
            259 => Self::SkirmishWaitForCommandbuttonAvailableAll,
            260 => Self::SkirmishWaitForCommandbuttonAvailablePartial,
            261 => Self::TeamSpinForFramecount,
            262 => Self::TeamAllUseCommandbuttonOnNamed,
            263 => Self::TeamAllUseCommandbuttonOnNearestEnemyUnit,
            264 => Self::TeamAllUseCommandbuttonOnNearestGarrisonedBuilding,
            265 => Self::TeamAllUseCommandbuttonOnNearestKindof,
            266 => Self::TeamAllUseCommandbuttonOnNearestEnemyBuilding,
            267 => Self::TeamAllUseCommandbuttonOnNearestEnemyBuildingClass,
            268 => Self::TeamAllUseCommandbuttonOnNearestObjecttype,
            269 => Self::TeamPartialUseCommandbutton,
            270 => Self::TeamCaptureNearestUnownedFactionUnit,
            271 => Self::PlayerCreateTeamFromCapturedUnits,
            272 => Self::PlayerAddSkillpoints,
            273 => Self::PlayerAddRanklevel,
            274 => Self::PlayerSetRanklevel,
            275 => Self::PlayerSetRanklevellimit,
            276 => Self::PlayerGrantScience,
            277 => Self::PlayerPurchaseScience,
            278 => Self::TeamHuntWithCommandButton,
            279 => Self::TeamWaitForNotContainedAll,
            280 => Self::TeamWaitForNotContainedPartial,
            281 => Self::TeamFollowWaypointsExact,
            282 => Self::NamedFollowWaypointsExact,
            283 => Self::TeamSetEmoticon,
            284 => Self::NamedSetEmoticon,
            285 => Self::AiPlayerBuildSupplyCenter,
            286 => Self::AiPlayerBuildUpgrade,
            287 => Self::ObjectlistAddobjecttype,
            288 => Self::ObjectlistRemoveobjecttype,
            289 => Self::MapRevealPermanentlyAtWaypoint,
            290 => Self::MapUndoRevealPermanentlyAtWaypoint,
            291 => Self::NamedSetStealthEnabled,
            292 => Self::TeamSetStealthEnabled,
            293 => Self::EvaSetEnabledDisabled,
            294 => Self::OptionsSetOcclusionMode,
            295 => Self::Localdefeat,
            296 => Self::OptionsSetDrawiconUiMode,
            297 => Self::OptionsSetParticleCapMode,
            298 => Self::PlayerScienceAvailability,
            299 => Self::UnitAffectObjectPanelFlags,
            300 => Self::TeamAffectObjectPanelFlags,
            301 => Self::PlayerSelectSkillset,
            302 => Self::ScriptingOverrideHulkLifetime,
            303 => Self::NamedFaceNamed,
            304 => Self::NamedFaceWaypoint,
            305 => Self::TeamFaceNamed,
            306 => Self::TeamFaceWaypoint,
            307 => Self::CommandbarRemoveButtonObjecttype,
            308 => Self::CommandbarAddButtonObjecttypeSlot,
            309 => Self::UnitSpawnNamedLocationOrientation,
            310 => Self::PlayerAffectReceivingExperience,
            311 => Self::PlayerExcludeFromScoreScreen,
            312 => Self::TeamGuardSupplyCenter,
            313 => Self::EnableScoring,
            314 => Self::DisableScoring,
            315 => Self::SoundSetVolume,
            316 => Self::SpeechSetVolume,
            317 => Self::DisableBorderShroud,
            318 => Self::EnableBorderShroud,
            319 => Self::ObjectAllowBonuses,
            320 => Self::SoundRemoveAllDisabled,
            321 => Self::SoundRemoveType,
            322 => Self::TeamGuardInTunnelNetwork,
            323 => Self::Quickvictory,
            324 => Self::SetInfantryLightingOverride,
            325 => Self::ResetInfantryLightingOverride,
            326 => Self::TeamDeleteLiving,
            327 => Self::ResizeViewGuardband,
            328 => Self::DeleteAllUnmanned,
            329 => Self::ChooseVictimAlwaysUsesNormal,
            330 => Self::CameraEnableSlaveMode,
            331 => Self::CameraDisableSlaveMode,
            332 => Self::CameraAddShakerAt,
            333 => Self::SetTrainHeld,
            334 => Self::NamedSetEvacLeftOrRight,
            335 => Self::EnableObjectSound,
            336 => Self::DisableObjectSound,
            337 => Self::NamedUseCommandbuttonAbilityUsingWaypointPath,
            338 => Self::NamedSetUnmannedStatus,
            339 => Self::TeamSetUnmannedStatus,
            340 => Self::NamedSetBoobytrapped,
            341 => Self::TeamSetBoobytrapped,
            342 => Self::ShowWeather,
            343 => Self::AiPlayerBuildTypeNearestTeam,
            344 => Self::NumItems,
            _ => return None,
        })
    }
}

/// Condition type enumeration matching the C++ ConditionType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ConditionType {
    ConditionFalse = 0,
    Counter = 1,
    Flag = 2,
    ConditionTrue = 3,
    TimerExpired = 4,
    PlayerAllDestroyed = 5,
    PlayerAllBuildfacilitiesDestroyed = 6,
    TeamInsideAreaPartially = 7,
    TeamDestroyed = 8,
    CameraMovementFinished = 9,
    TeamHasUnits = 10,
    TeamStateIs = 11,
    TeamStateIsNot = 12,
    NamedInsideArea = 13,
    NamedOutsideArea = 14,
    NamedDestroyed = 15,
    NamedNotDestroyed = 16,
    TeamInsideAreaEntirely = 17,
    TeamOutsideAreaEntirely = 18,
    NamedAttackedByObjecttype = 19,
    TeamAttackedByObjecttype = 20,
    NamedAttackedByPlayer = 21,
    TeamAttackedByPlayer = 22,
    BuiltByPlayer = 23,
    NamedCreated = 24,
    TeamCreated = 25,
    PlayerHasCredits = 26,
    NamedDiscovered = 27,
    TeamDiscovered = 28,
    MissionAttempts = 29,
    NamedOwnedByPlayer = 30,
    TeamOwnedByPlayer = 31,
    PlayerHasNOrFewerBuildings = 32,
    PlayerHasPower = 33,
    NamedReachedWaypointsEnd = 34,
    TeamReachedWaypointsEnd = 35,
    NamedSelected = 36,
    NamedEnteredArea = 37,
    NamedExitedArea = 38,
    TeamEnteredAreaEntirely = 39,
    TeamEnteredAreaPartially = 40,
    TeamExitedAreaEntirely = 41,
    TeamExitedAreaPartially = 42,
    MultiplayerAlliedVictory = 43,
    MultiplayerAlliedDefeat = 44,
    MultiplayerPlayerDefeat = 45,
    PlayerHasNoPower = 46,
    HasFinishedVideo = 47,
    HasFinishedSpeech = 48,
    HasFinishedAudio = 49,
    BuildingEnteredByPlayer = 50,
    EnemySighted = 51,
    UnitHealth = 52,
    BridgeRepaired = 53,
    BridgeBroken = 54,
    NamedDying = 55,
    NamedTotallyDead = 56,
    PlayerHasObjectComparison = 57,
    ObsoleteScript1 = 58,
    ObsoleteScript2 = 59,
    PlayerTriggeredSpecialPower = 60,
    PlayerCompletedSpecialPower = 61,
    PlayerMidwaySpecialPower = 62,
    PlayerTriggeredSpecialPowerFromNamed = 63,
    PlayerCompletedSpecialPowerFromNamed = 64,
    PlayerMidwaySpecialPowerFromNamed = 65,
    DefunctPlayerSelectedGeneral = 66,
    DefunctPlayerSelectedGeneralFromNamed = 67,
    PlayerBuiltUpgrade = 68,
    PlayerBuiltUpgradeFromNamed = 69,
    PlayerDestroyedNBuildingsPlayer = 70,
    UnitCompletedSequentialExecution = 71,
    TeamCompletedSequentialExecution = 72,
    PlayerHasComparisonUnitTypeInTriggerArea = 73,
    PlayerHasComparisonUnitKindInTriggerArea = 74,
    UnitEmptied = 75,
    TypeSighted = 76,
    NamedBuildingIsEmpty = 77,
    PlayerHasNOrFewerFactionBuildings = 78,
    UnitHasObjectStatus = 79,
    TeamAllHasObjectStatus = 80,
    TeamSomeHaveObjectStatus = 81,
    PlayerPowerComparePercent = 82,
    PlayerExcessPowerCompareValue = 83,
    SkirmishSpecialPowerReady = 84,
    SkirmishValueInArea = 85,
    SkirmishPlayerFaction = 86,
    SkirmishSuppliesValueWithinDistance = 87,
    SkirmishTechBuildingWithinDistance = 88,
    SkirmishCommandButtonReadyAll = 89,
    SkirmishCommandButtonReadyPartial = 90,
    SkirmishUnownedFactionUnitExists = 91,
    SkirmishPlayerHasPrerequisiteToBuild = 92,
    SkirmishPlayerHasComparisonGarrisoned = 93,
    SkirmishPlayerHasComparisonCapturedUnits = 94,
    SkirmishNamedAreaExist = 95,
    SkirmishPlayerHasUnitsInArea = 96,
    SkirmishPlayerHasBeenAttackedByPlayer = 97,
    SkirmishPlayerIsOutsideArea = 98,
    SkirmishPlayerHasDiscoveredPlayer = 99,
    PlayerAcquiredScience = 100,
    PlayerHasSciencepurchasepoints = 101,
    PlayerCanPurchaseScience = 102,
    MusicTrackHasCompleted = 103,
    PlayerLostObjectType = 104,
    SupplySourceSafe = 105,
    SupplySourceAttacked = 106,
    StartPositionIs = 107,
    NamedHasFreeContainerSlots = 108,
    NumItems = 109,
}

impl ConditionType {
    pub fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::ConditionFalse,
            1 => Self::Counter,
            2 => Self::Flag,
            3 => Self::ConditionTrue,
            4 => Self::TimerExpired,
            5 => Self::PlayerAllDestroyed,
            6 => Self::PlayerAllBuildfacilitiesDestroyed,
            7 => Self::TeamInsideAreaPartially,
            8 => Self::TeamDestroyed,
            9 => Self::CameraMovementFinished,
            10 => Self::TeamHasUnits,
            11 => Self::TeamStateIs,
            12 => Self::TeamStateIsNot,
            13 => Self::NamedInsideArea,
            14 => Self::NamedOutsideArea,
            15 => Self::NamedDestroyed,
            16 => Self::NamedNotDestroyed,
            17 => Self::TeamInsideAreaEntirely,
            18 => Self::TeamOutsideAreaEntirely,
            19 => Self::NamedAttackedByObjecttype,
            20 => Self::TeamAttackedByObjecttype,
            21 => Self::NamedAttackedByPlayer,
            22 => Self::TeamAttackedByPlayer,
            23 => Self::BuiltByPlayer,
            24 => Self::NamedCreated,
            25 => Self::TeamCreated,
            26 => Self::PlayerHasCredits,
            27 => Self::NamedDiscovered,
            28 => Self::TeamDiscovered,
            29 => Self::MissionAttempts,
            30 => Self::NamedOwnedByPlayer,
            31 => Self::TeamOwnedByPlayer,
            32 => Self::PlayerHasNOrFewerBuildings,
            33 => Self::PlayerHasPower,
            34 => Self::NamedReachedWaypointsEnd,
            35 => Self::TeamReachedWaypointsEnd,
            36 => Self::NamedSelected,
            37 => Self::NamedEnteredArea,
            38 => Self::NamedExitedArea,
            39 => Self::TeamEnteredAreaEntirely,
            40 => Self::TeamEnteredAreaPartially,
            41 => Self::TeamExitedAreaEntirely,
            42 => Self::TeamExitedAreaPartially,
            43 => Self::MultiplayerAlliedVictory,
            44 => Self::MultiplayerAlliedDefeat,
            45 => Self::MultiplayerPlayerDefeat,
            46 => Self::PlayerHasNoPower,
            47 => Self::HasFinishedVideo,
            48 => Self::HasFinishedSpeech,
            49 => Self::HasFinishedAudio,
            50 => Self::BuildingEnteredByPlayer,
            51 => Self::EnemySighted,
            52 => Self::UnitHealth,
            53 => Self::BridgeRepaired,
            54 => Self::BridgeBroken,
            55 => Self::NamedDying,
            56 => Self::NamedTotallyDead,
            57 => Self::PlayerHasObjectComparison,
            58 => Self::ObsoleteScript1,
            59 => Self::ObsoleteScript2,
            60 => Self::PlayerTriggeredSpecialPower,
            61 => Self::PlayerCompletedSpecialPower,
            62 => Self::PlayerMidwaySpecialPower,
            63 => Self::PlayerTriggeredSpecialPowerFromNamed,
            64 => Self::PlayerCompletedSpecialPowerFromNamed,
            65 => Self::PlayerMidwaySpecialPowerFromNamed,
            66 => Self::DefunctPlayerSelectedGeneral,
            67 => Self::DefunctPlayerSelectedGeneralFromNamed,
            68 => Self::PlayerBuiltUpgrade,
            69 => Self::PlayerBuiltUpgradeFromNamed,
            70 => Self::PlayerDestroyedNBuildingsPlayer,
            71 => Self::UnitCompletedSequentialExecution,
            72 => Self::TeamCompletedSequentialExecution,
            73 => Self::PlayerHasComparisonUnitTypeInTriggerArea,
            74 => Self::PlayerHasComparisonUnitKindInTriggerArea,
            75 => Self::UnitEmptied,
            76 => Self::TypeSighted,
            77 => Self::NamedBuildingIsEmpty,
            78 => Self::PlayerHasNOrFewerFactionBuildings,
            79 => Self::UnitHasObjectStatus,
            80 => Self::TeamAllHasObjectStatus,
            81 => Self::TeamSomeHaveObjectStatus,
            82 => Self::PlayerPowerComparePercent,
            83 => Self::PlayerExcessPowerCompareValue,
            84 => Self::SkirmishSpecialPowerReady,
            85 => Self::SkirmishValueInArea,
            86 => Self::SkirmishPlayerFaction,
            87 => Self::SkirmishSuppliesValueWithinDistance,
            88 => Self::SkirmishTechBuildingWithinDistance,
            89 => Self::SkirmishCommandButtonReadyAll,
            90 => Self::SkirmishCommandButtonReadyPartial,
            91 => Self::SkirmishUnownedFactionUnitExists,
            92 => Self::SkirmishPlayerHasPrerequisiteToBuild,
            93 => Self::SkirmishPlayerHasComparisonGarrisoned,
            94 => Self::SkirmishPlayerHasComparisonCapturedUnits,
            95 => Self::SkirmishNamedAreaExist,
            96 => Self::SkirmishPlayerHasUnitsInArea,
            97 => Self::SkirmishPlayerHasBeenAttackedByPlayer,
            98 => Self::SkirmishPlayerIsOutsideArea,
            99 => Self::SkirmishPlayerHasDiscoveredPlayer,
            100 => Self::PlayerAcquiredScience,
            101 => Self::PlayerHasSciencepurchasepoints,
            102 => Self::PlayerCanPurchaseScience,
            103 => Self::MusicTrackHasCompleted,
            104 => Self::PlayerLostObjectType,
            105 => Self::SupplySourceSafe,
            106 => Self::SupplySourceAttacked,
            107 => Self::StartPositionIs,
            108 => Self::NamedHasFreeContainerSlots,
            109 => Self::NumItems,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod script_enum_from_u32_tests {
    use super::{ConditionType, ScriptActionType};

    #[test]
    fn script_action_and_condition_from_u32_are_exhaustive_matches() {
        assert_eq!(
            ScriptActionType::from_u32(0),
            Some(ScriptActionType::DebugMessageBox)
        );
        assert_eq!(
            ScriptActionType::from_u32(344),
            Some(ScriptActionType::NumItems)
        );
        assert_eq!(ScriptActionType::from_u32(345), None);
        assert_eq!(
            ConditionType::from_u32(0),
            Some(ConditionType::ConditionFalse)
        );
        assert_eq!(ConditionType::from_u32(109), Some(ConditionType::NumItems));
        assert_eq!(ConditionType::from_u32(110), None);
        let impl_src = include_str!("core.rs");
        let action = impl_src
            .split("impl ScriptActionType")
            .nth(1)
            .and_then(|s| s.split("impl ConditionType").next())
            .unwrap_or("");
        let condition = impl_src
            .split("impl ConditionType")
            .nth(1)
            .and_then(|s| s.split("#[cfg(test)]").next())
            .unwrap_or("");
        assert!(!action.contains("mem::transmute"));
        assert!(!condition.contains("mem::transmute"));
    }
}

/// Parameter type enumeration matching C++ Parameter::ParameterType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ParameterType {
    Int = 0,
    Real = 1,
    Script = 2,
    Team = 3,
    Counter = 4,
    Flag = 5,
    Comparison = 6,
    Waypoint = 7,
    Boolean = 8,
    TriggerArea = 9,
    TextString = 10,
    Side = 11,
    Sound = 12,
    ScriptSubroutine = 13,
    Unit = 14,
    ObjectType = 15,
    Coord3D = 16,
    Angle = 17,
    TeamState = 18,
    Relation = 19,
    AiMood = 20,
    Dialog = 21,
    Music = 22,
    Movie = 23,
    WaypointPath = 24,
    LocalizedText = 25,
    Bridge = 26,
    KindOfParam = 27,
    AttackPrioritySet = 28,
    RadarEventType = 29,
    SpecialPower = 30,
    Science = 31,
    Upgrade = 32,
    CommandbuttonAbility = 33,
    Boundary = 34,
    Buildable = 35,
    SurfacesAllowed = 36,
    ShakeIntensity = 37,
    CommandButton = 38,
    FontName = 39,
    ObjectStatus = 40,
    CommandbuttonAllAbilities = 41,
    SkirmishWaypointPath = 42,
    Color = 43,
    Emoticon = 44,
    ObjectPanelFlag = 45,
    FactionName = 46,
    ObjectTypeList = 47,
    Revealname = 48,
    ScienceAvailability = 49,
    LeftOrRight = 50,
    Percent = 51,
    NumItems = 52,
}

impl ParameterType {
    pub fn from_u32(value: u32) -> Option<Self> {
        Some(match value {
            0 => Self::Int,
            1 => Self::Real,
            2 => Self::Script,
            3 => Self::Team,
            4 => Self::Counter,
            5 => Self::Flag,
            6 => Self::Comparison,
            7 => Self::Waypoint,
            8 => Self::Boolean,
            9 => Self::TriggerArea,
            10 => Self::TextString,
            11 => Self::Side,
            12 => Self::Sound,
            13 => Self::ScriptSubroutine,
            14 => Self::Unit,
            15 => Self::ObjectType,
            16 => Self::Coord3D,
            17 => Self::Angle,
            18 => Self::TeamState,
            19 => Self::Relation,
            20 => Self::AiMood,
            21 => Self::Dialog,
            22 => Self::Music,
            23 => Self::Movie,
            24 => Self::WaypointPath,
            25 => Self::LocalizedText,
            26 => Self::Bridge,
            27 => Self::KindOfParam,
            28 => Self::AttackPrioritySet,
            29 => Self::RadarEventType,
            30 => Self::SpecialPower,
            31 => Self::Science,
            32 => Self::Upgrade,
            33 => Self::CommandbuttonAbility,
            34 => Self::Boundary,
            35 => Self::Buildable,
            36 => Self::SurfacesAllowed,
            37 => Self::ShakeIntensity,
            38 => Self::CommandButton,
            39 => Self::FontName,
            40 => Self::ObjectStatus,
            41 => Self::CommandbuttonAllAbilities,
            42 => Self::SkirmishWaypointPath,
            43 => Self::Color,
            44 => Self::Emoticon,
            45 => Self::ObjectPanelFlag,
            46 => Self::FactionName,
            47 => Self::ObjectTypeList,
            48 => Self::Revealname,
            49 => Self::ScienceAvailability,
            50 => Self::LeftOrRight,
            51 => Self::Percent,
            52 => Self::NumItems,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod parameter_type_from_u32_tests {
    use super::ParameterType;

    #[test]
    fn parameter_type_from_u32_is_exhaustive_and_rejects_oob() {
        assert_eq!(ParameterType::from_u32(0), Some(ParameterType::Int));
        assert_eq!(ParameterType::from_u32(51), Some(ParameterType::Percent));
        assert_eq!(ParameterType::from_u32(52), Some(ParameterType::NumItems));
        assert_eq!(ParameterType::from_u32(53), None);
        let impl_src = include_str!("core.rs")
            .split("impl ParameterType")
            .nth(1)
            .and_then(|s| s.split("impl ComparisonType").next())
            .unwrap_or("");
        assert!(
            !impl_src.contains("mem::transmute"),
            "ParameterType::from_u32 must not transmute"
        );
    }
}

/// Comparison types matching C++ Parameter comparison types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ComparisonType {
    LessThan = 0,
    LessEqual = 1,
    Equal = 2,
    GreaterEqual = 3,
    Greater = 4,
    NotEqual = 5,
}

/// Relation types matching C++ Parameter relation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum RelationType {
    Enemy = 0,   // ENEMIES
    Neutral = 1, // NEUTRAL
    Friend = 2,  // ALLIES
}

/// Parameter class matching C++ Parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub param_type: ParameterType,
    pub initialized: bool,
    pub int_value: i32,
    pub real_value: f32,
    pub string_value: String,
    pub coord_value: Coord3D,
    pub object_status: ObjectStatusMaskType,
}

impl Parameter {
    pub fn new(param_type: ParameterType) -> Self {
        Self {
            param_type,
            initialized: false,
            int_value: 0,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_int(param_type: ParameterType, value: i32) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: value,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_real(param_type: ParameterType, value: f32) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: value,
            string_value: String::new(),
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_string(param_type: ParameterType, value: String) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: 0.0,
            string_value: value,
            coord_value: Coord3D::new(0.0, 0.0, 0.0),
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn with_coord(param_type: ParameterType, value: Coord3D) -> Self {
        Self {
            param_type,
            initialized: true,
            int_value: 0,
            real_value: 0.0,
            string_value: String::new(),
            coord_value: value,
            object_status: ObjectStatusMaskType::NONE,
        }
    }

    pub fn get_int(&self) -> i32 {
        self.int_value
    }

    pub fn get_real(&self) -> f32 {
        self.real_value
    }

    pub fn get_string(&self) -> &str {
        &self.string_value
    }

    pub fn get_coord(&self) -> Coord3D {
        self.coord_value
    }

    pub fn get_object_status(&self) -> ObjectStatusMaskType {
        self.object_status
    }

    pub fn get_parameter_type(&self) -> ParameterType {
        self.param_type
    }

    pub fn qualify(&mut self, qualifier: &str, player_template_name: &str, new_player_name: &str) {
        match self.param_type {
            ParameterType::Side => {
                let mut tmp = self.string_value.clone();
                tmp.push_str(qualifier);
                if tmp == player_template_name {
                    self.string_value = new_player_name.to_string();
                }
            }
            ParameterType::Team => {
                if self.string_value != THIS_TEAM {
                    self.string_value.push_str(qualifier);
                }
            }
            ParameterType::Script
            | ParameterType::Counter
            | ParameterType::Flag
            | ParameterType::ScriptSubroutine => {
                self.string_value.push_str(qualifier);
            }
            _ => {}
        }
    }

    pub fn write_parameter(&mut self, writer: &mut game_engine::common::system::DataChunkOutput) {
        writer.write_int(self.param_type as i32);
        if self.param_type == ParameterType::KindOfParam {
            self.string_value = kind_of_index_to_name(self.int_value).to_string();
        }
        if self.param_type == ParameterType::Coord3D {
            writer.write_real(self.coord_value.x);
            writer.write_real(self.coord_value.y);
            writer.write_real(self.coord_value.z);
        } else {
            writer.write_int(self.int_value);
            writer.write_real(self.real_value);
            writer.write_ascii_string(&self.string_value);
        }
    }

    pub fn read_parameter(input: &mut DataChunkInput) -> Option<Parameter> {
        let param_type = ParameterType::from_u32(input.read_int() as u32)?;
        let mut param = Parameter::new(param_type);
        param.initialized = true;
        if param_type == ParameterType::Coord3D {
            let x = input.read_real();
            let y = input.read_real();
            let z = input.read_real();
            param.coord_value = Coord3D::new(x, y, z);
        } else {
            param.int_value = input.read_int();
            param.real_value = input.read_real();
            param.string_value = input.read_ascii_string();
        }

        if param_type == ParameterType::ObjectType
            && param.string_value.starts_with("Fundamentalist")
        {
            let old = param.string_value.clone();
            let suffix = old.trim_start_matches("Fundamentalist");
            param.string_value = format!("GLA{suffix}");
        }

        if param_type == ParameterType::Upgrade
            && (param.string_value == "Upgrade_AmericaRangerCaptureBuilding"
                || param.string_value == "Upgrade_ChinaRedguardCaptureBuilding"
                || param.string_value == "Upgrade_GLARebelCaptureBuilding")
        {
            param.string_value = "Upgrade_InfantryCaptureBuilding".to_string();
        }

        if param_type == ParameterType::ObjectStatus {
            if let Some(mask) =
                ObjectStatusMaskType::from_case_insensitive_name(&param.string_value)
            {
                param.object_status = mask;
            }
        }

        if param_type == ParameterType::KindOfParam {
            if !param.string_value.is_empty() {
                if let Some(kind) = parse_kind_of(&param.string_value) {
                    if let Some(index) = ALL_KIND_OF.iter().position(|k| *k == kind) {
                        param.int_value = index as i32;
                    }
                }
            } else {
                param.string_value = kind_of_index_to_name(param.int_value).to_string();
            }
        }

        Some(param)
    }
}

/// Template class matching C++ Template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub ui_name: String,
    pub ui_name2: String,
    pub internal_name: String,
    pub internal_name_key: u32,
    pub num_ui_strings: usize,
    pub ui_strings: Vec<String>,
    pub num_parameters: usize,
    pub parameters: Vec<ParameterType>,
    pub help_text: String,
    #[cfg(feature = "count_script_usage")]
    pub num_times_used: i32,
    #[cfg(feature = "count_script_usage")]
    pub first_map_used: String,
}

impl Template {
    pub fn new() -> Self {
        Self {
            ui_name: String::new(),
            ui_name2: String::new(),
            internal_name: String::new(),
            internal_name_key: 0,
            num_ui_strings: 0,
            ui_strings: Vec::new(),
            num_parameters: 0,
            parameters: Vec::new(),
            help_text: String::new(),
            #[cfg(feature = "count_script_usage")]
            num_times_used: 0,
            #[cfg(feature = "count_script_usage")]
            first_map_used: String::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.ui_name
    }

    pub fn get_name2(&self) -> &str {
        &self.ui_name2
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parameters
    }

    pub fn get_parameter_type(&self, index: usize) -> Option<ParameterType> {
        self.parameters.get(index).copied()
    }
}

/// Condition template matching C++ ConditionTemplate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionTemplate {
    pub base: Template,
}

impl ConditionTemplate {
    pub fn new() -> Self {
        Self {
            base: Template::new(),
        }
    }

    pub fn qualify(
        &mut self,
        _qualifier: &str,
        _player_template_name: &str,
        _new_player_name: &str,
    ) {
    }
}

/// Action template matching C++ ActionTemplate  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    pub base: Template,
}

impl ActionTemplate {
    pub fn new() -> Self {
        Self {
            base: Template::new(),
        }
    }
}

/// Condition class matching C++ Condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub condition_type: ConditionType,
    pub num_parms: usize,
    pub parameters: Vec<Option<Parameter>>,
    pub next_and_condition: Option<Box<Condition>>,
    pub has_warnings: bool,
    pub custom_data: i32,
    pub custom_frame: u32,
}

impl Condition {
    pub fn new(condition_type: ConditionType) -> Self {
        Self {
            condition_type,
            num_parms: 0,
            parameters: vec![None; MAX_PARMS],
            next_and_condition: None,
            has_warnings: false,
            custom_data: 0,
            custom_frame: 0,
        }
    }

    pub fn get_condition_type(&self) -> ConditionType {
        self.condition_type
    }

    pub fn get_parameter(&self, index: usize) -> Option<&Parameter> {
        if index < self.parameters.len() {
            self.parameters[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parms
    }

    pub fn get_next(&self) -> Option<&Condition> {
        self.next_and_condition.as_ref().map(|c| c.as_ref())
    }

    pub fn set_next_condition(&mut self, condition: Option<Box<Condition>>) {
        self.next_and_condition = condition;
    }

    pub fn add_parameter(&mut self, parameter: Parameter) -> GameLogicResult<()> {
        if self.num_parms >= MAX_PARMS {
            return Err(GameLogicError::Configuration(
                "Maximum number of parameters exceeded".to_string(),
            ));
        }

        self.parameters[self.num_parms] = Some(parameter);
        self.num_parms += 1;
        Ok(())
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<Condition> {
        let mut new_condition = Condition {
            condition_type: self.condition_type,
            num_parms: self.num_parms,
            parameters: self.parameters.clone(),
            next_and_condition: None,
            has_warnings: self.has_warnings,
            custom_data: self.custom_data,
            custom_frame: self.custom_frame,
        };

        for index in 0..new_condition.num_parms {
            if let Some(param) = new_condition
                .parameters
                .get_mut(index)
                .and_then(|p| p.as_mut())
            {
                param.qualify(qualifier, player_template_name, new_player_name);
            }
        }

        if let Some(next) = &self.next_and_condition {
            new_condition.next_and_condition =
                Some(next.duplicate_and_qualify(qualifier, player_template_name, new_player_name));
        }

        Box::new(new_condition)
    }

    pub fn write_condition_data_chunk(
        mut condition: Option<&Condition>,
        output: &mut DataChunkOutput,
    ) {
        while let Some(cur) = condition {
            output.open_data_chunk("Condition", K_SCRIPT_CONDITION_VERSION_4);
            output.write_int(cur.condition_type as i32);
            let mut key = NameKeyGenerator::name_to_key("Bogus");
            if let Ok(engine_lock) = get_script_engine().read() {
                if let Some(engine) = engine_lock.as_ref() {
                    if let Some(template) =
                        engine.get_condition_template(cur.condition_type as usize)
                    {
                        key = template.base.internal_name_key;
                    }
                }
            }
            output.write_name_key(key);
            output.write_int(cur.num_parms as i32);
            for index in 0..cur.num_parms {
                if let Some(param) = cur.parameters.get(index).and_then(|p| p.as_ref()) {
                    let mut param = param.clone();
                    param.write_parameter(output);
                }
            }
            output.close_data_chunk();
            condition = cur.next_and_condition.as_deref();
        }
    }
}

fn kind_of_index_to_name(index: i32) -> &'static str {
    let idx = index.max(0) as usize;
    if idx >= ALL_KIND_OF.len() {
        return "";
    }
    match ALL_KIND_OF[idx] {
        KindOf::Selectable => "SELECTABLE",
        KindOf::Unit => "UNIT",
        KindOf::Building => "BUILDING",
        KindOf::Vehicle => "VEHICLE",
        KindOf::Infantry => "INFANTRY",
        KindOf::Aircraft => "AIRCRAFT",
        KindOf::Drone => "DRONE",
        KindOf::CliffJumper => "CLIFFJUMPER",
        KindOf::Structure => "STRUCTURE",
        KindOf::Weapon => "WEAPON",
        KindOf::Projectile => "PROJECTILE",
        KindOf::CanSeeThrough => "CAN_SEE_THROUGH",
        KindOf::AlwaysSelectable => "ALWAYS_SELECTABLE",
        KindOf::Crate => "CRATE",
        KindOf::ResourceNode => "RESOURCE_NODE",
        KindOf::SupplySourceOnPreview => "SUPPLY_SOURCE_ON_PREVIEW",
        KindOf::SupplySource => "SUPPLY_SOURCE",
        KindOf::Disguiser => "DISGUISER",
        KindOf::PortableStructure => "PORTABLE_STRUCTURE",
        KindOf::TechBuilding => "TECH_BUILDING",
        KindOf::Bridge => "BRIDGE",
        KindOf::Barrier => "BARRIER",
        KindOf::Civilian => "CIVILIAN",
        KindOf::Destructible => "DESTRUCTIBLE",
        KindOf::CanCrossBridges => "CAN_CROSS_BRIDGES",
        KindOf::Amphibious => "AMPHIBIOUS",
        KindOf::AmphibiousTransport => "AMPHIBIOUS_TRANSPORT",
        KindOf::CanCapture => "CAN_CAPTURE",
        KindOf::Saboteur => "SABOTEUR",
        KindOf::Hacker => "HACKER",
        KindOf::Hero => "HERO",
        KindOf::KeyStructure => "KEY_STRUCTURE",
        KindOf::CommandCenter => "COMMAND_CENTER",
        KindOf::Prison => "PRISON",
        KindOf::CollectsPrisonBounty => "COLLECTS_PRISON_BOUNTY",
        KindOf::PowTruck => "POW_TRUCK",
        KindOf::PowerPlant => "POWER_PLANT",
        KindOf::Refinery => "REFINERY",
        KindOf::Factory => "FACTORY",
        KindOf::Defense => "DEFENSE",
        KindOf::Shrubbery => "SHRUBBERY",
        KindOf::Dozer => "DOZER",
        KindOf::Harvester => "HARVESTER",
        KindOf::Hulk => "HULK",
        KindOf::Salvager => "SALVAGER",
        KindOf::WeaponSalvager => "WEAPON_SALVAGER",
        KindOf::ArmorSalvager => "ARMOR_SALVAGER",
        KindOf::AircraftCarrier => "AIRCRAFT_CARRIER",
        KindOf::FSBarracks => "FS_BARRACKS",
        KindOf::FSWarfactory => "FS_WARFACTORY",
        KindOf::FSAirfield => "FS_AIRFIELD",
        KindOf::FSInternetCenter => "FS_INTERNET_CENTER",
        KindOf::FSPower => "FS_POWER",
        KindOf::FSBaseDefense => "FS_BASE_DEFENSE",
        KindOf::FSSupplyDropzone => "FS_SUPPLY_DROPZONE",
        KindOf::FSSupplyCenter => "FS_SUPPLY_CENTER",
        KindOf::FSSuperweapon => "FS_SUPERWEAPON",
        KindOf::FSStrategyCenter => "FS_STRATEGY_CENTER",
        KindOf::CountsForVictory => "COUNTS_FOR_VICTORY",
        KindOf::Mine => "MINE",
        KindOf::CleanupHazard => "CLEANUP_HAZARD",
        KindOf::HealPad => "HEAL_PAD",
        KindOf::WaveGuide => "WAVE_GUIDE",
        KindOf::BridgeTower => "BRIDGE_TOWER",
        KindOf::Immobile => "IMMOBILE",
        KindOf::BoobyTrap => "BOOBY_TRAP",
        KindOf::CanBeRepulsed => "CAN_BE_REPULSED",
        KindOf::EmpHardened => "EMP_HARDENED",
        KindOf::SpawnsAreTheWeapons => "SPAWNS_ARE_THE_WEAPONS",
        KindOf::IgnoreDockingBones => "IGNORE_DOCKING_BONES",
        KindOf::CanSurrender => "CAN_SURRENDER",
        KindOf::RepairPad => "REPAIR_PAD",
        KindOf::RejectUnmanned => "REJECT_UNMANNED",
        KindOf::IgnoredInGui => "IGNORED_IN_GUI",
        KindOf::MobNexus => "MOB_NEXUS",
        KindOf::Capturable => "CAPTURABLE",
        KindOf::ImmuneToCapture => "IMMUNE_TO_CAPTURE",
        KindOf::CashGenerator => "CASH_GENERATOR",
        KindOf::RebuildHole => "REBUILD_HOLE",
        KindOf::FSTechnology => "FS_TECHNOLOGY",
        KindOf::GarrisonableUntilDestroyed => "GARRISONABLE_UNTIL_DESTROYED",
        KindOf::NoGarrison => "NO_GARRISON",
        _ => "",
    }
}

/// OR Condition class matching C++ OrCondition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrCondition {
    pub next_or: Option<Box<OrCondition>>,
    pub first_and: Option<Box<Condition>>,
}

impl OrCondition {
    pub fn new() -> Self {
        Self {
            next_or: None,
            first_and: None,
        }
    }

    pub fn get_next_or_condition(&self) -> Option<&OrCondition> {
        self.next_or.as_ref().map(|c| c.as_ref())
    }

    pub fn get_first_and_condition(&self) -> Option<&Condition> {
        self.first_and.as_ref().map(|c| c.as_ref())
    }

    pub fn set_next_or_condition(&mut self, condition: Option<Box<OrCondition>>) {
        self.next_or = condition;
    }

    pub fn set_first_and_condition(&mut self, condition: Option<Box<Condition>>) {
        self.first_and = condition;
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<OrCondition> {
        let mut new_or = OrCondition::new();
        if let Some(first_and) = &self.first_and {
            new_or.first_and = Some(first_and.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }

        let mut current_src = &self.next_or;
        let mut current_dst = &mut new_or;
        while let Some(src) = current_src {
            let mut next_or = OrCondition::new();
            if let Some(first_and) = &src.first_and {
                next_or.first_and = Some(first_and.duplicate_and_qualify(
                    qualifier,
                    player_template_name,
                    new_player_name,
                ));
            }
            current_dst.next_or = Some(Box::new(next_or));
            current_dst = current_dst.next_or.as_mut().unwrap();
            current_src = &src.next_or;
        }

        Box::new(new_or)
    }

    pub fn write_or_condition_data_chunk(
        mut condition: Option<&OrCondition>,
        output: &mut DataChunkOutput,
    ) {
        while let Some(cur) = condition {
            output.open_data_chunk("OrCondition", K_SCRIPT_OR_CONDITION_DATA_VERSION_1);
            if let Some(first_and) = cur.first_and.as_deref() {
                Condition::write_condition_data_chunk(Some(first_and), output);
            }
            output.close_data_chunk();
            condition = cur.next_or.as_deref();
        }
    }
}

/// Script Action class matching C++ ScriptAction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAction {
    pub action_type: ScriptActionType,
    pub num_parms: usize,
    pub parameters: Vec<Option<Parameter>>,
    pub next_action: Option<Box<ScriptAction>>,
    pub has_warnings: bool,
}

impl ScriptAction {
    pub fn new(action_type: ScriptActionType) -> Self {
        Self {
            action_type,
            num_parms: 0,
            parameters: vec![None; MAX_PARMS],
            next_action: None,
            has_warnings: false,
        }
    }

    pub fn get_action_type(&self) -> ScriptActionType {
        self.action_type
    }

    pub fn get_parameter(&self, index: usize) -> Option<&Parameter> {
        if index < self.parameters.len() {
            self.parameters[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_num_parameters(&self) -> usize {
        self.num_parms
    }

    pub fn get_next(&self) -> Option<&ScriptAction> {
        self.next_action.as_ref().map(|a| a.as_ref())
    }

    pub fn set_next_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.next_action = action;
    }

    pub fn add_parameter(&mut self, parameter: Parameter) -> GameLogicResult<()> {
        if self.num_parms >= MAX_PARMS {
            return Err(GameLogicError::Configuration(
                "Maximum number of parameters exceeded".to_string(),
            ));
        }

        self.parameters[self.num_parms] = Some(parameter);
        self.num_parms += 1;
        Ok(())
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptAction> {
        let mut new_action = ScriptAction {
            action_type: self.action_type,
            num_parms: self.num_parms,
            parameters: self.parameters.clone(),
            next_action: None,
            has_warnings: self.has_warnings,
        };

        for index in 0..new_action.num_parms {
            if let Some(param) = new_action
                .parameters
                .get_mut(index)
                .and_then(|p| p.as_mut())
            {
                param.qualify(qualifier, player_template_name, new_player_name);
            }
        }

        if let Some(next) = &self.next_action {
            new_action.next_action =
                Some(next.duplicate_and_qualify(qualifier, player_template_name, new_player_name));
        }

        Box::new(new_action)
    }

    pub fn write_action_data_chunk(
        mut action: Option<&ScriptAction>,
        output: &mut DataChunkOutput,
        label: &str,
    ) {
        while let Some(cur) = action {
            output.open_data_chunk(label, K_SCRIPT_ACTION_VERSION_2);
            output.write_int(cur.action_type as i32);
            let mut key = NameKeyGenerator::name_to_key("Bogus");
            if let Ok(engine_lock) = get_script_engine().read() {
                if let Some(engine) = engine_lock.as_ref() {
                    if let Some(template) = engine.get_action_template(cur.action_type as usize) {
                        key = template.base.internal_name_key;
                    }
                }
            }
            output.write_name_key(key);
            output.write_int(cur.num_parms as i32);
            for index in 0..cur.num_parms {
                if let Some(param) = cur.parameters.get(index).and_then(|p| p.as_ref()) {
                    let mut param = param.clone();
                    param.write_parameter(output);
                }
            }
            output.close_data_chunk();
            action = cur.next_action.as_deref();
        }
    }
}

impl Script {
    pub fn append_action(&mut self, action: Box<ScriptAction>) {
        let mut current = self.action.as_mut();
        while let Some(node) = current {
            if node.next_action.is_none() {
                node.next_action = Some(action);
                return;
            }
            current = node.next_action.as_mut();
        }
        self.action = Some(action);
    }

    pub fn append_action_false(&mut self, action: Box<ScriptAction>) {
        let mut current = self.action_false.as_mut();
        while let Some(node) = current {
            if node.next_action.is_none() {
                node.next_action = Some(action);
                return;
            }
            current = node.next_action.as_mut();
        }
        self.action_false = Some(action);
    }

    pub fn append_or_condition(&mut self, condition: Box<OrCondition>) {
        let mut current = self.condition.as_mut();
        while let Some(node) = current {
            if node.next_or.is_none() {
                node.next_or = Some(condition);
                return;
            }
            current = node.next_or.as_mut();
        }
        self.condition = Some(condition);
    }
}

/// Script class matching C++ Script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub script_name: String,
    pub comment: String,
    pub condition_comment: String,
    pub action_comment: String,
    pub delay_evaluation_seconds: i32,
    pub is_active: bool,
    pub is_one_shot: bool,
    pub is_subroutine: bool,
    pub easy: bool,
    pub normal: bool,
    pub hard: bool,
    pub condition: Option<Box<OrCondition>>,
    pub action: Option<Box<ScriptAction>>,
    pub action_false: Option<Box<ScriptAction>>,
    pub next_script: Option<Box<Script>>,
    // Runtime fields
    pub frame_to_evaluate_at: u32,
    pub has_warnings: bool,
    pub condition_team_name: String,
    pub condition_time: f32,
    pub cur_time: f32,
    pub condition_executed_count: i32,
}

impl Script {
    pub fn new() -> Self {
        Self {
            script_name: String::new(),
            comment: String::new(),
            condition_comment: String::new(),
            action_comment: String::new(),
            delay_evaluation_seconds: 0,
            is_active: true,
            is_one_shot: true,
            is_subroutine: false,
            easy: true,
            normal: true,
            hard: true,
            condition: None,
            action: None,
            action_false: None,
            next_script: None,
            frame_to_evaluate_at: 0,
            has_warnings: false,
            condition_team_name: String::new(),
            condition_time: 0.0,
            cur_time: 0.0,
            condition_executed_count: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.script_name
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn is_one_shot(&self) -> bool {
        self.is_one_shot
    }

    pub fn is_subroutine(&self) -> bool {
        self.is_subroutine
    }

    pub fn get_or_condition(&self) -> Option<&OrCondition> {
        self.condition.as_ref().map(|c| c.as_ref())
    }

    pub fn get_action(&self) -> Option<&ScriptAction> {
        self.action.as_ref().map(|a| a.as_ref())
    }

    pub fn get_false_action(&self) -> Option<&ScriptAction> {
        self.action_false.as_ref().map(|a| a.as_ref())
    }

    pub fn get_next(&self) -> Option<&Script> {
        self.next_script.as_ref().map(|s| s.as_ref())
    }

    pub fn set_name(&mut self, name: String) {
        self.script_name = name;
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn set_one_shot(&mut self, one_shot: bool) {
        self.is_one_shot = one_shot;
    }

    pub fn set_subroutine(&mut self, subroutine: bool) {
        self.is_subroutine = subroutine;
    }

    pub fn set_or_condition(&mut self, condition: Option<Box<OrCondition>>) {
        self.condition = condition;
    }

    pub fn set_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.action = action;
    }

    pub fn set_false_action(&mut self, action: Option<Box<ScriptAction>>) {
        self.action_false = action;
    }

    pub fn set_next_script(&mut self, script: Option<Box<Script>>) {
        self.next_script = script;
    }

    pub fn increment_condition_count(&mut self) {
        self.condition_executed_count += 1;
    }

    pub fn add_to_condition_time(&mut self, time: f32) {
        self.condition_time += time;
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<Script> {
        let mut new_script = Script::new();
        new_script.script_name = format!("{}{}", self.script_name, qualifier);
        new_script.comment = self.comment.clone();
        new_script.condition_comment = self.condition_comment.clone();
        new_script.action_comment = self.action_comment.clone();
        new_script.delay_evaluation_seconds = self.delay_evaluation_seconds;
        new_script.is_active = self.is_active;
        new_script.is_one_shot = self.is_one_shot;
        new_script.is_subroutine = self.is_subroutine;
        new_script.easy = self.easy;
        new_script.normal = self.normal;
        new_script.hard = self.hard;

        if let Some(condition) = &self.condition {
            new_script.condition = Some(condition.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }
        if let Some(action) = &self.action {
            new_script.action = Some(action.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }
        if let Some(action_false) = &self.action_false {
            new_script.action_false = Some(action_false.duplicate_and_qualify(
                qualifier,
                player_template_name,
                new_player_name,
            ));
        }

        Box::new(new_script)
    }

    pub fn write_script_data_chunk(mut script: Option<&Script>, output: &mut DataChunkOutput) {
        while let Some(cur) = script {
            output.open_data_chunk("Script", K_SCRIPT_DATA_VERSION_2);
            output.write_ascii_string(&cur.script_name);
            output.write_ascii_string(&cur.comment);
            output.write_ascii_string(&cur.condition_comment);
            output.write_ascii_string(&cur.action_comment);
            output.write_byte(cur.is_active as u8);
            output.write_byte(cur.is_one_shot as u8);
            output.write_byte(cur.easy as u8);
            output.write_byte(cur.normal as u8);
            output.write_byte(cur.hard as u8);
            output.write_byte(cur.is_subroutine as u8);
            output.write_int(cur.delay_evaluation_seconds);
            if let Some(condition) = cur.condition.as_deref() {
                OrCondition::write_or_condition_data_chunk(Some(condition), output);
            }
            if let Some(action) = cur.action.as_deref() {
                ScriptAction::write_action_data_chunk(Some(action), output, "ScriptAction");
            }
            if let Some(action_false) = cur.action_false.as_deref() {
                ScriptAction::write_action_data_chunk(
                    Some(action_false),
                    output,
                    "ScriptActionFalse",
                );
            }
            output.close_data_chunk();
            script = cur.next_script.as_deref();
        }
    }
}

/// Script Group class matching C++ ScriptGroup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptGroup {
    pub first_script: Option<Box<Script>>,
    pub group_name: String,
    pub is_group_active: bool,
    pub is_group_subroutine: bool,
    pub next_group: Option<Box<ScriptGroup>>,
    pub has_warnings: bool,
}

impl ScriptGroup {
    pub fn new() -> Self {
        let id = SCRIPT_GROUP_ID.fetch_add(1, Ordering::Relaxed) + 1;
        Self {
            first_script: None,
            group_name: format!("Script Group {}", id),
            is_group_active: true,
            is_group_subroutine: false,
            next_group: None,
            has_warnings: false,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.group_name
    }

    pub fn is_active(&self) -> bool {
        self.is_group_active
    }

    pub fn is_subroutine(&self) -> bool {
        self.is_group_subroutine
    }

    pub fn get_script(&self) -> Option<&Script> {
        self.first_script.as_ref().map(|s| s.as_ref())
    }

    pub fn get_next(&self) -> Option<&ScriptGroup> {
        self.next_group.as_ref().map(|g| g.as_ref())
    }

    pub fn set_name(&mut self, name: String) {
        self.group_name = name;
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_group_active = active;
    }

    pub fn set_subroutine(&mut self, subroutine: bool) {
        self.is_group_subroutine = subroutine;
    }

    pub fn append_script(&mut self, script: Box<Script>) {
        let mut current = self.first_script.as_mut();
        while let Some(node) = current {
            if node.next_script.is_none() {
                node.next_script = Some(script);
                return;
            }
            current = node.next_script.as_mut();
        }
        self.first_script = Some(script);
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptGroup> {
        let mut new_group = ScriptGroup {
            first_script: None,
            group_name: format!("{}{}", self.group_name, qualifier),
            is_group_active: self.is_group_active,
            is_group_subroutine: self.is_group_subroutine,
            next_group: None,
            has_warnings: self.has_warnings,
        };

        let mut current_src = &self.first_script;
        let mut tail: Option<&mut Box<Script>> = None;
        while let Some(src) = current_src {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = tail {
                tail_ref.next_script = Some(dup);
                tail = tail_ref.next_script.as_mut();
            } else {
                new_group.first_script = Some(dup);
                tail = new_group.first_script.as_mut();
            }
            current_src = &src.next_script;
        }

        Box::new(new_group)
    }

    pub fn write_group_data_chunk(mut group: Option<&ScriptGroup>, output: &mut DataChunkOutput) {
        while let Some(cur) = group {
            output.open_data_chunk("ScriptGroup", K_SCRIPT_GROUP_DATA_VERSION_2);
            output.write_ascii_string(&cur.group_name);
            output.write_byte(cur.is_group_active as u8);
            output.write_byte(cur.is_group_subroutine as u8);
            if let Some(script) = cur.first_script.as_deref() {
                Script::write_script_data_chunk(Some(script), output);
            }
            output.close_data_chunk();
            group = cur.next_group.as_deref();
        }
    }
}

/// Script List class matching C++ ScriptList
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptList {
    pub first_group: Option<Box<ScriptGroup>>,
    pub first_script: Option<Box<Script>>,
}

impl ScriptList {
    pub fn new() -> Self {
        Self {
            first_group: None,
            first_script: None,
        }
    }

    pub fn get_script_group(&self) -> Option<&ScriptGroup> {
        self.first_group.as_ref().map(|g| g.as_ref())
    }

    pub fn get_script(&self) -> Option<&Script> {
        self.first_script.as_ref().map(|s| s.as_ref())
    }

    pub fn append_script(&mut self, script: Box<Script>) {
        let mut current = self.first_script.as_mut();
        while let Some(node) = current {
            if node.next_script.is_none() {
                node.next_script = Some(script);
                return;
            }
            current = node.next_script.as_mut();
        }
        self.first_script = Some(script);
    }

    /// C++ `ScriptList::addScript(pScr, ndx)` — insert at `ndx` (0 prepends).
    pub fn add_script(&mut self, mut script: Box<Script>, mut ndx: i32) {
        script.next_script = None;
        if ndx <= 0 || self.first_script.is_none() {
            script.next_script = self.first_script.take();
            self.first_script = Some(script);
            return;
        }
        let mut cur = self.first_script.as_mut().expect("first_script checked");
        ndx -= 1;
        while ndx > 0 && cur.next_script.is_some() {
            cur = cur.next_script.as_mut().expect("next_script checked");
            ndx -= 1;
        }
        script.next_script = cur.next_script.take();
        cur.next_script = Some(script);
    }

    pub fn append_group(&mut self, group: Box<ScriptGroup>) {
        let mut current = self.first_group.as_mut();
        while let Some(node) = current {
            if node.next_group.is_none() {
                node.next_group = Some(group);
                return;
            }
            current = node.next_group.as_mut();
        }
        self.first_group = Some(group);
    }

    pub fn duplicate_and_qualify(
        &self,
        qualifier: &str,
        player_template_name: &str,
        new_player_name: &str,
    ) -> Box<ScriptList> {
        let mut new_list = Box::new(ScriptList::new());

        let mut current_src_group = &self.first_group;
        let mut group_tail: Option<&mut Box<ScriptGroup>> = None;
        while let Some(src) = current_src_group {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = group_tail {
                tail_ref.next_group = Some(dup);
                group_tail = tail_ref.next_group.as_mut();
            } else {
                new_list.first_group = Some(dup);
                group_tail = new_list.first_group.as_mut();
            }
            current_src_group = &src.next_group;
        }

        let mut current_src_script = &self.first_script;
        let mut script_tail: Option<&mut Box<Script>> = None;
        while let Some(src) = current_src_script {
            let dup = src.duplicate_and_qualify(qualifier, player_template_name, new_player_name);
            if let Some(tail_ref) = script_tail {
                tail_ref.next_script = Some(dup);
                script_tail = tail_ref.next_script.as_mut();
            } else {
                new_list.first_script = Some(dup);
                script_tail = new_list.first_script.as_mut();
            }
            current_src_script = &src.next_script;
        }

        new_list
    }

    pub fn parse_scripts_data_chunk(input: &mut DataChunkInput) -> Vec<Box<ScriptList>> {
        let mut read_info = ScriptListReadInfo::default();
        input.register_parser(
            "ScriptList",
            "PlayerScriptsList",
            parse_script_list_data_chunk,
        );
        let _ = input.parse(&mut read_info);
        read_info.lists
    }

    pub fn write_scripts_data_chunk(
        output: &mut DataChunkOutput,
        script_lists: &[Option<&ScriptList>],
    ) {
        output.open_data_chunk("PlayerScriptsList", K_SCRIPTS_DATA_VERSION_1);
        for list in script_lists {
            output.open_data_chunk("ScriptList", K_SCRIPT_LIST_DATA_VERSION_1);
            if let Some(list) = list {
                list.write_script_list_data_chunk(output);
            }
            output.close_data_chunk();
        }
        output.close_data_chunk();
    }

    pub fn write_script_list_data_chunk(&self, output: &mut DataChunkOutput) {
        if let Some(script) = self.first_script.as_deref() {
            Script::write_script_data_chunk(Some(script), output);
        }
        if let Some(group) = self.first_group.as_deref() {
            ScriptGroup::write_group_data_chunk(Some(group), output);
        }
    }
}

impl XferSnapshot for Script {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut active = self.is_active;
        xfer.xfer_bool(&mut active)?;
        self.is_active = active;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

impl XferSnapshot for ScriptGroup {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if version >= 2 {
            let mut active = self.is_group_active;
            xfer.xfer_bool(&mut active)?;
            self.is_group_active = active;
        }

        let mut script_count: u16 = 0;
        let mut cursor = self.first_script.as_deref();
        while let Some(script) = cursor {
            script_count = script_count.saturating_add(1);
            cursor = script.next_script.as_deref();
        }

        let count_verify = script_count;
        xfer.xfer_unsigned_short(&mut script_count)?;
        let mut remaining = script_count;

        let mut current = self.first_script.as_mut();
        while let Some(script) = current {
            if remaining == 0 {
                break;
            }
            script.as_mut().xfer(xfer)?;
            remaining = remaining.saturating_sub(1);
            current = script.next_script.as_mut();
        }

        if count_verify != script_count {
            // Attempt to recover by consuming any extra serialized scripts.
            while remaining > 0 {
                let mut temp = Script::new();
                temp.xfer(xfer)?;
                remaining = remaining.saturating_sub(1);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

impl XferSnapshot for ScriptList {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut script_count: u16 = 0;
        let mut cursor = self.first_script.as_deref();
        while let Some(script) = cursor {
            script_count = script_count.saturating_add(1);
            cursor = script.next_script.as_deref();
        }
        let count_verify = script_count;
        xfer.xfer_unsigned_short(&mut script_count)?;
        let mut remaining = script_count;

        let mut current = self.first_script.as_mut();
        while let Some(script) = current {
            if remaining == 0 {
                break;
            }
            script.as_mut().xfer(xfer)?;
            remaining = remaining.saturating_sub(1);
            current = script.next_script.as_mut();
        }

        if count_verify != script_count {
            while remaining > 0 {
                let mut temp = Script::new();
                temp.xfer(xfer)?;
                remaining = remaining.saturating_sub(1);
            }
        }

        let mut group_count: u16 = 0;
        let mut gcur = self.first_group.as_deref();
        while let Some(group) = gcur {
            group_count = group_count.saturating_add(1);
            gcur = group.next_group.as_deref();
        }
        let group_verify = group_count;
        xfer.xfer_unsigned_short(&mut group_count)?;
        let mut group_remaining = group_count;

        let mut current_group = self.first_group.as_mut();
        while let Some(group) = current_group {
            if group_remaining == 0 {
                break;
            }
            group.as_mut().xfer(xfer)?;
            group_remaining = group_remaining.saturating_sub(1);
            current_group = group.next_group.as_mut();
        }

        if group_verify != group_count {
            while group_remaining > 0 {
                let mut temp = ScriptGroup::new();
                temp.xfer(xfer)?;
                group_remaining = group_remaining.saturating_sub(1);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ScriptListReadInfo {
    pub lists: Vec<Box<ScriptList>>,
}

fn user_data_mut<T: 'static>(user_data: &mut dyn std::any::Any) -> Option<&mut T> {
    user_data.downcast_mut::<T>()
}

fn parse_script_list_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(read_info) = user_data_mut::<ScriptListReadInfo>(user_data) else {
        return false;
    };

    let mut script_list = Box::new(ScriptList::new());
    input.register_parser("Script", &info.label, parse_script_from_list_data_chunk);
    input.register_parser("ScriptGroup", &info.label, parse_group_data_chunk);
    let _ = input.parse(script_list.as_mut());
    read_info.lists.push(script_list);
    true
}

fn parse_script_from_list_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(list) = user_data_mut::<ScriptList>(user_data) else {
        return false;
    };
    if let Some(script) = parse_script(input, info) {
        list.append_script(Box::new(script));
    }
    input.at_end_of_chunk()
}

fn parse_script_from_group_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(group) = user_data_mut::<ScriptGroup>(user_data) else {
        return false;
    };
    if let Some(script) = parse_script(input, info) {
        group.append_script(Box::new(script));
    }
    input.at_end_of_chunk()
}

fn parse_group_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(list) = user_data_mut::<ScriptList>(user_data) else {
        return false;
    };
    let mut group = Box::new(ScriptGroup::new());
    group.group_name = input.read_ascii_string();
    group.is_group_active = input.read_byte() != 0;
    if info.version >= 2 {
        group.is_group_subroutine = input.read_byte() != 0;
    }
    input.register_parser("Script", &info.label, parse_script_from_group_data_chunk);
    let _ = input.parse(group.as_mut());
    list.append_group(group);
    true
}

fn parse_script(input: &mut DataChunkInput, info: &DataChunkInfo) -> Option<Script> {
    let mut script = Script::new();
    script.script_name = input.read_ascii_string();
    script.comment = input.read_ascii_string();
    script.condition_comment = input.read_ascii_string();
    script.action_comment = input.read_ascii_string();
    script.is_active = input.read_byte() != 0;
    script.is_one_shot = input.read_byte() != 0;
    script.easy = input.read_byte() != 0;
    script.normal = input.read_byte() != 0;
    script.hard = input.read_byte() != 0;
    script.is_subroutine = input.read_byte() != 0;
    if info.version >= 2 {
        script.delay_evaluation_seconds = input.read_int();
    }

    input.register_parser("OrCondition", "Script", parse_or_condition_data_chunk);
    input.register_parser("ScriptAction", "Script", parse_action_data_chunk);
    input.register_parser("ScriptActionFalse", "Script", parse_action_false_data_chunk);
    let _ = input.parse(&mut script);
    Some(script)
}

fn parse_or_condition_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<Script>(user_data) else {
        return false;
    };
    let mut or_cond = Box::new(OrCondition::new());
    input.register_parser("Condition", &info.label, parse_condition_data_chunk);
    let _ = input.parse(or_cond.as_mut());
    script.append_or_condition(or_cond);
    true
}

fn parse_condition_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(or_cond) = user_data_mut::<OrCondition>(user_data) else {
        return false;
    };

    let mut condition_type =
        ConditionType::from_u32(input.read_int() as u32).unwrap_or(ConditionType::ConditionFalse);

    if info.version >= 4 {
        let name_key = input.read_name_key();
        let mut matched = false;
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_condition_template(condition_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(ct) = engine.find_condition_type_by_name_key(name_key) {
                        condition_type = ct;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            condition_type = ConditionType::ConditionFalse;
        }
    }

    let num_parms = input.read_int().max(0) as usize;
    let mut condition = Box::new(Condition::new(condition_type));
    condition.num_parms = num_parms;
    for idx in 0..num_parms.min(MAX_PARMS) {
        condition.parameters[idx] = Parameter::read_parameter(input);
    }

    if condition.condition_type == ConditionType::SkirmishSpecialPowerReady
        && condition.num_parms == 1
    {
        condition.num_parms = 2;
        if let Some(first) = condition.parameters[0].clone() {
            condition.parameters[1] = Some(first);
        }
        condition.parameters[0] = Some(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.to_string(),
        ));
    }

    let mut tail = &mut or_cond.first_and;
    while let Some(node) = tail {
        tail = &mut node.next_and_condition;
    }
    *tail = Some(condition);

    input.at_end_of_chunk()
}

fn parse_action(input: &mut DataChunkInput, info: &DataChunkInfo) -> Box<ScriptAction> {
    let mut action_type =
        ScriptActionType::from_u32(input.read_int() as u32).unwrap_or(ScriptActionType::NoOp);

    if info.version >= 2 {
        let name_key = input.read_name_key();
        let mut matched = false;
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_action_template(action_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(at) = engine.find_action_type_by_name_key(name_key) {
                        action_type = at;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            action_type = ScriptActionType::NoOp;
        }
    }

    let mut action = Box::new(ScriptAction::new(action_type));
    let num_parms = input.read_int().max(0) as usize;
    action.num_parms = num_parms;
    for idx in 0..num_parms.min(MAX_PARMS) {
        action.parameters[idx] = Parameter::read_parameter(input);
    }

    if action.action_type == ScriptActionType::SkirmishFireSpecialPowerAtMostCost
        && action.num_parms == 1
    {
        action.num_parms = 2;
        if let Some(first) = action.parameters[0].clone() {
            action.parameters[1] = Some(first);
        }
        action.parameters[0] = Some(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.to_string(),
        ));
    }

    if action.action_type == ScriptActionType::TeamFollowWaypoints && action.num_parms == 2 {
        action.num_parms = 3;
        action.parameters[2] = Some(Parameter::with_int(ParameterType::Boolean, 1));
    }

    if action.action_type == ScriptActionType::SkirmishBuildBaseDefenseFront
        && action.num_parms == 1
    {
        let flank = action.parameters[0]
            .as_ref()
            .map(|p| p.get_int() != 0)
            .unwrap_or(false);
        action.parameters[0] = None;
        action.num_parms = 0;
        if flank {
            action.action_type = ScriptActionType::SkirmishBuildBaseDefenseFlank;
        }
    }

    if matches!(
        action.action_type,
        ScriptActionType::NamedSetAttitude | ScriptActionType::TeamSetAttitude
    ) && action.num_parms >= 2
    {
        if let Some(param) = action.parameters[1].clone() {
            if param.param_type == ParameterType::Int {
                action.parameters[1] =
                    Some(Parameter::with_int(ParameterType::AiMood, param.int_value));
            }
        }
    }

    if matches!(
        action.action_type,
        ScriptActionType::MapRevealAtWaypoint | ScriptActionType::MapShroudAtWaypoint
    ) && action.num_parms == 2
    {
        action.num_parms = 3;
        action.parameters[2] = Some(Parameter::new(ParameterType::Side));
    }

    if matches!(
        action.action_type,
        ScriptActionType::MapRevealAll
            | ScriptActionType::MapRevealAllPerm
            | ScriptActionType::MapRevealAllUndoPerm
            | ScriptActionType::MapShroudAll
    ) && action.num_parms == 0
    {
        action.num_parms = 1;
        action.parameters[0] = Some(Parameter::new(ParameterType::Side));
    }

    if action.action_type == ScriptActionType::SpeechPlay && action.num_parms == 1 {
        action.num_parms = 2;
        action.parameters[1] = Some(Parameter::with_int(ParameterType::Boolean, 1));
    }

    if matches!(
        action.action_type,
        ScriptActionType::CameraModSetFinalZoom | ScriptActionType::CameraModSetFinalPitch
    ) && action.num_parms == 1
    {
        action.num_parms = 3;
        action.parameters[1] = Some(Parameter::with_real(ParameterType::Percent, 0.0));
        action.parameters[2] = Some(Parameter::with_real(ParameterType::Percent, 0.0));
    }

    if matches!(
        action.action_type,
        ScriptActionType::MoveCameraTo
            | ScriptActionType::MoveCameraAlongWaypointPath
            | ScriptActionType::CameraLookTowardObject
    ) && action.num_parms == 3
    {
        action.num_parms = 5;
        action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
        action.parameters[4] = Some(Parameter::with_real(ParameterType::Real, 0.0));
    }

    if matches!(
        action.action_type,
        ScriptActionType::ResetCamera
            | ScriptActionType::ZoomCamera
            | ScriptActionType::PitchCamera
            | ScriptActionType::RotateCamera
    ) && action.num_parms == 2
    {
        action.num_parms = 4;
        action.parameters[2] = Some(Parameter::with_real(ParameterType::Real, 0.0));
        action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
    }

    if action.action_type == ScriptActionType::CameraLookTowardWaypoint {
        if action.num_parms == 2 {
            action.num_parms = 5;
            action.parameters[2] = Some(Parameter::with_real(ParameterType::Real, 0.0));
            action.parameters[3] = Some(Parameter::with_real(ParameterType::Real, 0.0));
            action.parameters[4] = Some(Parameter::with_int(ParameterType::Boolean, 0));
        } else if action.num_parms == 4 {
            action.num_parms = 5;
            action.parameters[4] = Some(Parameter::with_int(ParameterType::Boolean, 0));
        }
    }

    action
}

fn parse_action_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<Script>(user_data) else {
        return false;
    };
    let action = parse_action(input, info);
    script.append_action(action);
    input.at_end_of_chunk()
}

fn parse_action_false_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(script) = user_data_mut::<Script>(user_data) else {
        return false;
    };
    let action = parse_action(input, info);
    script.append_action_false(action);
    input.at_end_of_chunk()
}

pub fn parse_player_scripts_list_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(read_info) = user_data_mut::<ScriptListReadInfo>(user_data) else {
        return false;
    };
    input.register_parser("ScriptList", &info.label, parse_script_list_data_chunk);
    input.parse(read_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::with_int(ParameterType::Int, 42);
        assert_eq!(param.get_int(), 42);
        assert_eq!(param.get_parameter_type(), ParameterType::Int);
        assert!(param.initialized);
    }

    #[test]
    fn test_script_action_creation() {
        let mut action = ScriptAction::new(ScriptActionType::Victory);
        assert_eq!(action.get_action_type(), ScriptActionType::Victory);
        assert_eq!(action.get_num_parameters(), 0);

        let param = Parameter::with_string(ParameterType::TextString, "Victory!".to_string());
        action.add_parameter(param).unwrap();
        assert_eq!(action.get_num_parameters(), 1);
    }

    #[test]
    fn test_condition_creation() {
        let mut condition = Condition::new(ConditionType::PlayerAllDestroyed);
        assert_eq!(
            condition.get_condition_type(),
            ConditionType::PlayerAllDestroyed
        );
        assert_eq!(condition.get_num_parameters(), 0);

        let param = Parameter::with_string(ParameterType::Side, "Player_1".to_string());
        condition.add_parameter(param).unwrap();
        assert_eq!(condition.get_num_parameters(), 1);
    }

    #[test]
    fn test_script_creation() {
        let script = Script::new();
        assert!(script.is_active());
        assert!(script.is_one_shot());
        assert!(!script.is_subroutine());
        assert_eq!(script.get_name(), "");
    }

    #[test]
    fn test_coord3d() {
        let coord = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);
    }

    #[test]
    fn add_script_zero_prepends_like_cpp() {
        let mut list = ScriptList::new();
        let mut first = Script::new();
        first.set_name("MapScript".into());
        list.append_script(Box::new(first));
        let mut mp = Script::new();
        mp.set_name("MultiplayerVictory".into());
        list.add_script(Box::new(mp), 0);
        assert_eq!(list.get_script().unwrap().get_name(), "MultiplayerVictory");
        assert_eq!(
            list.get_script().unwrap().get_next().unwrap().get_name(),
            "MapScript"
        );
    }
}
