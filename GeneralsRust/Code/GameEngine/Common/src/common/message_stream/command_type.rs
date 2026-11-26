#![allow(non_upper_case_globals)]

//! IDs and display names for the legacy command interface.

use crate::common::ascii_string::AsciiString;
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Numeric IDs used by the legacy command queues and their callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
#[repr(u16)]
pub enum CommandType {
    // System messages
    Invalid = 0,
    FrameTick = 1,

    // Raw mouse messages - match C++ MSG_RAW_MOUSE_* exactly
    RawMouseBegin = 100,
    RawMousePosition = 101,
    RawMouseLeftButtonDown = 102,
    RawMouseLeftDoubleClick = 103,
    RawMouseLeftButtonUp = 104,
    RawMouseLeftClick = 105,
    RawMouseLeftDrag = 106,
    RawMouseMiddleButtonDown = 107,
    RawMouseMiddleDoubleClick = 108,
    RawMouseMiddleButtonUp = 109,
    RawMouseMiddleDrag = 110,
    RawMouseRightButtonDown = 111,
    RawMouseRightDoubleClick = 112,
    RawMouseRightButtonUp = 113,
    RawMouseRightDrag = 114,
    RawMouseWheel = 115,
    RawMouseEnd = 116,

    // Keyboard messages
    RawKeyDown = 120,
    RawKeyUp = 121,

    // Refined mouse messages - preferred over raw messages
    MouseLeftClick = 130,
    MouseLeftDoubleClick = 131,
    MouseMiddleClick = 132,
    MouseMiddleDoubleClick = 133,
    MouseRightClick = 134,
    MouseRightDoubleClick = 135,

    // Game state messages
    ClearGameData = 140,
    NewGame = 141,

    // Meta messages (virtual keystrokes) - match C++ MSG_META_* exactly
    BeginMetaMessages = 200,
    MetaSaveView1 = 201,
    MetaSaveView2 = 202,
    MetaSaveView3 = 203,
    MetaSaveView4 = 204,
    MetaSaveView5 = 205,
    MetaSaveView6 = 206,
    MetaSaveView7 = 207,
    MetaSaveView8 = 208,
    MetaViewView1 = 209,
    MetaViewView2 = 210,
    MetaViewView3 = 211,
    MetaViewView4 = 212,
    MetaViewView5 = 213,
    MetaViewView6 = 214,
    MetaViewView7 = 215,
    MetaViewView8 = 216,

    // Team creation/selection meta messages
    MetaCreateTeam0 = 220,
    MetaCreateTeam1 = 221,
    MetaCreateTeam2 = 222,
    MetaCreateTeam3 = 223,
    MetaCreateTeam4 = 224,
    MetaCreateTeam5 = 225,
    MetaCreateTeam6 = 226,
    MetaCreateTeam7 = 227,
    MetaCreateTeam8 = 228,
    MetaCreateTeam9 = 229,

    MetaSelectTeam0 = 230,
    MetaSelectTeam1 = 231,
    MetaSelectTeam2 = 232,
    MetaSelectTeam3 = 233,
    MetaSelectTeam4 = 234,
    MetaSelectTeam5 = 235,
    MetaSelectTeam6 = 236,
    MetaSelectTeam7 = 237,
    MetaSelectTeam8 = 238,
    MetaSelectTeam9 = 239,

    MetaAddTeam0 = 240,
    MetaAddTeam1 = 241,
    MetaAddTeam2 = 242,
    MetaAddTeam3 = 243,
    MetaAddTeam4 = 244,
    MetaAddTeam5 = 245,
    MetaAddTeam6 = 246,
    MetaAddTeam7 = 247,
    MetaAddTeam8 = 248,
    MetaAddTeam9 = 249,

    // Unit selection and management
    MetaSelectMatchingUnits = 250,
    MetaSelectNextUnit = 251,
    MetaSelectPrevUnit = 252,
    MetaSelectNextWorker = 253,
    MetaSelectPrevWorker = 254,
    MetaViewCommandCenter = 255,
    MetaViewLastRadarEvent = 256,
    MetaSelectHero = 257,
    MetaSelectAll = 258,
    MetaSelectAllAircraft = 259,

    // Unit commands
    MetaScatter = 260,
    MetaStop = 261,
    MetaDeploy = 262,
    MetaCreateFormation = 263,
    MetaFollow = 264,

    // Communication
    MetaChatPlayers = 270,
    MetaChatAllies = 271,
    MetaChatEveryone = 272,
    MetaDiplomacy = 273,
    MetaOptions = 274,
    MetaHelp = 275,

    // Mode toggles
    MetaBeginPathBuild = 280,
    MetaEndPathBuild = 281,
    MetaBeginForceAttack = 282,
    MetaEndForceAttack = 283,
    MetaBeginForceMove = 284,
    MetaEndForceMove = 285,
    MetaBeginWaypoints = 286,
    MetaEndWaypoints = 287,
    MetaBeginPreferSelection = 288,
    MetaEndPreferSelection = 289,

    // Camera control
    MetaBeginCameraRotateLeft = 290,
    MetaEndCameraRotateLeft = 291,
    MetaBeginCameraRotateRight = 292,
    MetaEndCameraRotateRight = 293,
    MetaBeginCameraZoomIn = 294,
    MetaEndCameraZoomIn = 295,
    MetaBeginCameraZoomOut = 296,
    MetaEndCameraZoomOut = 297,
    MetaCameraReset = 298,

    // Beacons
    MetaPlaceBeacon = 299,
    MetaRemoveBeacon = 300,

    EndMetaMessages = 400,

    // Command hints - help system understand what commands are valid
    MouseoverDrawableHint = 500,
    MouseoverLocationHint = 501,
    ValidGuiCommandHint = 502,
    InvalidGuiCommandHint = 503,
    AreaSelectionHint = 504,

    // Action hints
    DoAttackObjectHint = 510,
    ImpossibleAttackHint = 511,
    DoForceAttackObjectHint = 512,
    DoForceAttackGroundHint = 513,
    GetRepairedHint = 514,
    GetHealedHint = 515,
    DoRepairHint = 516,
    ResumeConstructionHint = 517,
    EnterHint = 518,
    DockHint = 519,
    DoMoveToHint = 520,
    DoAttackMoveToHint = 521,
    AddWaypointHint = 522,

    // Special ability hints
    HijackHint = 530,
    SabotageHint = 531,
    FirebombHint = 532,
    ConvertToCarbombHint = 533,
    CaptureBuildingHint = 534,
    SnipeVehicleHint = 535,
    DefectorHint = 536,
    SetRallyPointHint = 537,
    DoSpecialPowerOverrideDestinationHint = 538,
    DoSalvageHint = 539,
    DoInvalidHint = 540,
    DoAttackObjectAfterMovingHint = 541,
    HackHint = 542,

    // Network messages start at 1000 - matches C++ MSG_BEGIN_NETWORK_MESSAGES exactly
    BeginNetworkMessages = 1000,

    // Selection commands - match C++ exactly
    CreateSelectedGroup = 1001,
    CreateSelectedGroupNoSound = 1002,
    DestroySelectedGroup = 1003,
    RemoveFromSelectedGroup = 1004,
    SelectedGroupCommand = 1005,

    // Team hotkey commands - match C++ exactly
    CreateTeam0 = 1010,
    CreateTeam1 = 1011,
    CreateTeam2 = 1012,
    CreateTeam3 = 1013,
    CreateTeam4 = 1014,
    CreateTeam5 = 1015,
    CreateTeam6 = 1016,
    CreateTeam7 = 1017,
    CreateTeam8 = 1018,
    CreateTeam9 = 1019,

    SelectTeam0 = 1020,
    SelectTeam1 = 1021,
    SelectTeam2 = 1022,
    SelectTeam3 = 1023,
    SelectTeam4 = 1024,
    SelectTeam5 = 1025,
    SelectTeam6 = 1026,
    SelectTeam7 = 1027,
    SelectTeam8 = 1028,
    SelectTeam9 = 1029,

    AddTeam0 = 1030,
    AddTeam1 = 1031,
    AddTeam2 = 1032,
    AddTeam3 = 1033,
    AddTeam4 = 1034,
    AddTeam5 = 1035,
    AddTeam6 = 1036,
    AddTeam7 = 1037,
    AddTeam8 = 1038,
    AddTeam9 = 1039,

    // Combat commands - match C++ exactly
    DoAttackSquad = 1040,
    DoWeapon = 1041,
    DoWeaponAtLocation = 1042,
    DoWeaponAtObject = 1043,
    DoSpecialPower = 1044,
    DoSpecialPowerAtLocation = 1045,
    DoSpecialPowerAtObject = 1046,

    // Building and production - match C++ exactly
    SetRallyPoint = 1050,
    PurchaseScience = 1051,
    QueueUpgrade = 1052,
    CancelUpgrade = 1053,
    QueueUnitCreate = 1054,
    CancelUnitCreate = 1055,
    DozerConstruct = 1056,
    DozerConstructLine = 1057,
    DozerCancelConstruct = 1058,
    Sell = 1059,

    // Unit actions - match C++ exactly
    Exit = 1060,
    Evacuate = 1061,
    ExecuteRailedTransport = 1062,
    CombatDropAtLocation = 1063,
    CombatDropAtObject = 1064,

    // Selection and targeting
    AreaSelection = 1070,
    DoAttackObject = 1071,
    DoForceAttackObject = 1072,
    DoForceAttackGround = 1073,
    GetRepaired = 1074,
    GetHealed = 1075,
    DoRepair = 1076,
    ResumeConstruction = 1077,
    Enter = 1078,
    Dock = 1079,

    // Movement commands - match C++ exactly
    DoMoveTo = 1080,
    DoAttackMoveTo = 1081,
    DoForceMoveTo = 1082,
    AddWaypoint = 1083,
    DoGuardPosition = 1084,
    DoGuardObject = 1085,
    DoStop = 1086,
    DoScatter = 1087,

    // Special abilities - match C++ exactly
    InternetHack = 1090,
    DoCheer = 1091,
    ToggleOvercharge = 1092,
    SwitchWeapons = 1093,
    ConvertToCarbomb = 1094,
    CaptureBuilding = 1095,
    DisableVehicleHack = 1096,
    StealCashHack = 1097,
    DisableBuildingHack = 1098,
    SnipeVehicle = 1099,
    DoSpecialPowerOverrideDestination = 1100,
    DoSalvage = 1101,
    PickUpPrisoner = 1102,
    ReturnToPrison = 1103,

    // UI and game state
    ClearInGamePopupMessage = 1110,
    PlaceBeacon = 1111,
    RemoveBeacon = 1112,
    SetBeaconText = 1113,
    SetReplayCamera = 1114,
    SelfDestruct = 1115,
    CreateFormation = 1116,
    LogicCrc = 1117,
    SetMineClearingDetail = 1118,
    EnableRetaliationMode = 1119,

    // Debug commands (debug/internal builds only)
    BeginDebugNetworkMessages = 1900,
    DebugKillSelection = 1901,
    DebugHurtObject = 1902,
    DebugKillObject = 1903,

    EndNetworkMessages = 1999,

    // Server to Client messages
    Timestamp = 2000,
    ObjectCreated = 2001,
    ObjectDestroyed = 2002,
    ObjectPosition = 2003,
    ObjectOrientation = 2004,
    ObjectJoinedTeam = 2005,

    Count = 2100,
}

impl CommandType {
    /// Convert command type to string for debugging - matches C++ getCommandTypeAsAsciiString
    pub fn as_ascii_string(&self) -> AsciiString {
        match self {
            CommandType::Invalid => AsciiString::from("MSG_INVALID"),
            CommandType::FrameTick => AsciiString::from("MSG_FRAME_TICK"),
            CommandType::RawMousePosition => AsciiString::from("MSG_RAW_MOUSE_POSITION"),
            CommandType::RawMouseLeftButtonDown => {
                AsciiString::from("MSG_RAW_MOUSE_LEFT_BUTTON_DOWN")
            }
            CommandType::RawMouseLeftClick => AsciiString::from("MSG_RAW_MOUSE_LEFT_CLICK"),
            CommandType::RawKeyDown => AsciiString::from("MSG_RAW_KEY_DOWN"),
            CommandType::RawKeyUp => AsciiString::from("MSG_RAW_KEY_UP"),
            CommandType::DoMoveTo => AsciiString::from("MSG_DO_MOVETO"),
            CommandType::DoAttackObject => AsciiString::from("MSG_DO_ATTACK_OBJECT"),
            CommandType::DoStop => AsciiString::from("MSG_DO_STOP"),
            CommandType::CreateSelectedGroup => AsciiString::from("MSG_CREATE_SELECTED_GROUP"),
            CommandType::AreaSelection => AsciiString::from("MSG_AREA_SELECTION"),
            CommandType::QueueUnitCreate => AsciiString::from("MSG_QUEUE_UNIT_CREATE"),
            CommandType::DozerConstruct => AsciiString::from("MSG_DOZER_CONSTRUCT"),
            CommandType::Sell => AsciiString::from("MSG_SELL"),
            _ => AsciiString::from(&format!("COMMAND_TYPE_{:?}", *self as u16)),
        }
    }

    /// Check if this is a network message that gets sent over multiplayer
    pub fn is_network_message(&self) -> bool {
        let value = *self as u16;
        value >= CommandType::BeginNetworkMessages as u16
            && value < CommandType::EndNetworkMessages as u16
    }

    /// Check if this is a meta message (virtual keystroke)
    pub fn is_meta_message(&self) -> bool {
        let value = *self as u16;
        value > CommandType::BeginMetaMessages as u16 && value < CommandType::EndMetaMessages as u16
    }

    /// Check if this is a raw mouse message
    pub fn is_raw_mouse_message(&self) -> bool {
        let value = *self as u16;
        value > CommandType::RawMouseBegin as u16 && value < CommandType::RawMouseEnd as u16
    }

    // Const aliases for backward compatibility with different naming conventions
    pub const SpecialPower: CommandType = CommandType::DoSpecialPower;
    pub const SwitchWeapon: CommandType = CommandType::SwitchWeapons;
    pub const FireWeapon: CommandType = CommandType::DoAttackObject;
    pub const HijackVehicle: CommandType = CommandType::Enter;
    pub const ConvertToCarBomb: CommandType = CommandType::ConvertToCarbomb;
    pub const SabotageBuilding: CommandType = CommandType::Enter;
}
