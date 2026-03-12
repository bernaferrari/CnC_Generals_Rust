////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Base Command System - Core command types and functionality
//!
//! This module provides the fundamental command system that exactly matches
//! the C++ GameMessage and Command system from the original game.
//! All RTS player commands, AI commands, and script commands flow through this system.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

// Import common types - these should match C++ exactly
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, ICoord2D, IRegion2D, Int, ObjectID, PlayerMaskType,
    Real, UnsignedByte, UnsignedInt, UnsignedShort, WideChar,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Maximum arguments per command - matches C++ GameMessage limit
pub const MAX_COMMAND_ARGUMENTS: usize = 255;

/// Command argument data types - matches C++ GameMessageArgumentDataType exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandArgumentDataType {
    Integer,
    Real,
    Boolean,
    ObjectID,
    DrawableId,
    TeamId,
    Location,
    Pixel,
    PixelRegion,
    Timestamp,
    WideChar,
    AsciiString,
    Unknown,
}

/// Command argument type union - matches C++ GameMessageArgumentType exactly
#[derive(Debug, Clone)]
pub enum CommandArgumentType {
    Integer(Int),
    Real(Real),
    Boolean(Bool),
    ObjectID(ObjectID),
    DrawableId(DrawableID),
    TeamId(UnsignedInt),
    SquadId(UnsignedInt),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(UnsignedInt),
    WideChar(WideChar),
    AsciiString(AsciiString),
}

impl CommandArgumentType {
    /// Get the data type of this argument
    pub fn get_data_type(&self) -> CommandArgumentDataType {
        match self {
            CommandArgumentType::Integer(_) => CommandArgumentDataType::Integer,
            CommandArgumentType::Real(_) => CommandArgumentDataType::Real,
            CommandArgumentType::Boolean(_) => CommandArgumentDataType::Boolean,
            CommandArgumentType::ObjectID(_) => CommandArgumentDataType::ObjectID,
            CommandArgumentType::DrawableId(_) => CommandArgumentDataType::DrawableId,
            CommandArgumentType::TeamId(_) => CommandArgumentDataType::TeamId,
            CommandArgumentType::SquadId(_) => CommandArgumentDataType::TeamId,
            CommandArgumentType::Location(_) => CommandArgumentDataType::Location,
            CommandArgumentType::Pixel(_) => CommandArgumentDataType::Pixel,
            CommandArgumentType::PixelRegion(_) => CommandArgumentDataType::PixelRegion,
            CommandArgumentType::Timestamp(_) => CommandArgumentDataType::Timestamp,
            CommandArgumentType::WideChar(_) => CommandArgumentDataType::WideChar,
            CommandArgumentType::AsciiString(_) => CommandArgumentDataType::AsciiString,
        }
    }
}

/// Command types - matches C++ GameMessage::Type enum exactly
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

/// Base Command class - matches C++ GameMessage exactly
#[derive(Debug, Clone)]
pub struct Command {
    /// Message type - what kind of command this is
    command_type: CommandType,

    /// Arguments for this command
    arguments: Vec<CommandArgumentType>,

    /// Player who issued this command
    player_index: Int,

    /// Unique command ID for tracking
    id: UnsignedInt,

    /// Timestamp when command was created
    timestamp: SystemTime,

    /// Execution frame (for network synchronization)
    execution_frame: UnsignedInt,

    /// Command priority
    priority: Int,
}

impl Command {
    /// Create new command - matches C++ GameMessage constructor
    pub fn new(command_type: CommandType) -> Self {
        Self {
            command_type,
            arguments: Vec::new(),
            player_index: -1,
            id: Self::generate_id(),
            timestamp: SystemTime::now(),
            execution_frame: 0,
            priority: 0,
        }
    }

    /// Generate unique command ID
    fn generate_id() -> UnsignedInt {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Get command type
    pub fn get_type(&self) -> CommandType {
        self.command_type
    }

    /// Get argument count - matches C++ getArgumentCount
    pub fn get_argument_count(&self) -> UnsignedByte {
        self.arguments.len() as UnsignedByte
    }

    /// Get player index - matches C++ getPlayerIndex
    pub fn get_player_index(&self) -> Int {
        self.player_index
    }

    /// Set player index - matches C++ friend_setPlayerIndex
    pub fn set_player_index(&mut self, player_index: Int) {
        self.player_index = player_index;
    }

    /// Get command identifier
    pub fn get_id(&self) -> UnsignedInt {
        self.id
    }

    /// Get execution frame
    pub fn get_execution_frame(&self) -> UnsignedInt {
        self.execution_frame
    }

    /// Set execution frame
    pub fn set_execution_frame(&mut self, frame: UnsignedInt) {
        self.execution_frame = frame;
    }

    /// Get command as string for debugging - matches C++ getCommandAsAsciiString
    pub fn get_command_as_ascii_string(&self) -> AsciiString {
        self.command_type.as_ascii_string()
    }

    // Argument methods - match C++ GameMessage methods exactly

    /// Append integer argument - matches C++ appendIntegerArgument
    pub fn append_integer_argument(&mut self, arg: Int) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Integer(arg));
        }
    }

    /// Append real argument - matches C++ appendRealArgument
    pub fn append_real_argument(&mut self, arg: Real) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Real(arg));
        }
    }

    /// Append boolean argument - matches C++ appendBooleanArgument
    pub fn append_boolean_argument(&mut self, arg: Bool) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Boolean(arg));
        }
    }

    /// Append object ID argument - matches C++ appendObjectIDArgument
    pub fn append_object_id_argument(&mut self, arg: ObjectID) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::ObjectID(arg));
        }
    }

    /// Append drawable ID argument - matches C++ appendDrawableIDArgument
    pub fn append_drawable_id_argument(&mut self, arg: DrawableID) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::DrawableId(arg));
        }
    }

    /// Append team ID argument - matches C++ appendTeamIDArgument
    pub fn append_team_id_argument(&mut self, arg: UnsignedInt) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::TeamId(arg));
        }
    }

    /// Append location argument - matches C++ appendLocationArgument
    pub fn append_location_argument(&mut self, arg: Coord3D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Location(arg));
        }
    }

    /// Append pixel argument - matches C++ appendPixelArgument
    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Pixel(arg));
        }
    }

    /// Append pixel region argument - matches C++ appendPixelRegionArgument
    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::PixelRegion(arg));
        }
    }

    /// Append timestamp argument - matches C++ appendTimestampArgument
    pub fn append_timestamp_argument(&mut self, arg: UnsignedInt) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Timestamp(arg));
        }
    }

    /// Append wide char argument - matches C++ appendWideCharArgument
    pub fn append_wide_char_argument(&mut self, arg: WideChar) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::WideChar(arg));
        }
    }

    /// Append ASCII string argument
    pub fn append_ascii_string_argument(&mut self, arg: AsciiString) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::AsciiString(arg));
        }
    }

    /// Get argument at index - matches C++ getArgument
    pub fn get_argument(&self, arg_index: Int) -> Option<&CommandArgumentType> {
        if arg_index >= 0 && (arg_index as usize) < self.arguments.len() {
            Some(&self.arguments[arg_index as usize])
        } else {
            None
        }
    }

    /// Get argument data type at index - matches C++ getArgumentDataType
    pub fn get_argument_data_type(&self, arg_index: Int) -> CommandArgumentDataType {
        if let Some(arg) = self.get_argument(arg_index) {
            arg.get_data_type()
        } else {
            CommandArgumentDataType::Unknown
        }
    }

    /// Get sort number for command ordering - matches C++ getSortNumber
    pub fn get_sort_number(&self) -> Int {
        self.priority
    }

    /// Set priority for command ordering
    pub fn set_priority(&mut self, priority: Int) {
        self.priority = priority;
    }
}

/// Command validation results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandValidation {
    Valid,
    InvalidType,
    InvalidArguments,
    InvalidPlayer,
    InvalidGameState,
    InsufficientResources,
    InvalidTarget,
    NotAllowed,
}

/// Command validator trait - allows custom validation logic
pub trait CommandValidator {
    /// Validate a command before execution
    fn validate_command(&self, command: &Command) -> CommandValidation;

    /// Check if player can execute this command type
    fn can_player_execute(&self, player_id: Int, command_type: CommandType) -> bool;
}

/// Default command validator with basic validation
pub struct DefaultCommandValidator;

impl CommandValidator for DefaultCommandValidator {
    fn validate_command(&self, command: &Command) -> CommandValidation {
        // Basic validation - check command type
        match command.get_type() {
            CommandType::Invalid => CommandValidation::InvalidType,
            CommandType::Count => CommandValidation::InvalidType,
            _ => CommandValidation::Valid,
        }
    }

    fn can_player_execute(&self, _player_id: Int, command_type: CommandType) -> bool {
        // Meta messages should not be sent over network
        if command_type.is_meta_message() {
            return false;
        }

        // Network messages are allowed
        command_type.is_network_message()
    }
}

/// Common command creation utilities
pub mod command_builder {
    use super::*;

    /// Create move to position command - matches common C++ pattern
    pub fn create_move_to_position(
        objects: Vec<ObjectID>,
        position: Coord3D,
        player_id: Int,
    ) -> Command {
        let mut command = Command::new(CommandType::DoMoveTo);
        command.set_player_index(player_id);

        // First argument is position
        command.append_location_argument(position);

        // Then add all object IDs
        for object_id in objects {
            command.append_object_id_argument(object_id);
        }

        command
    }

    /// Create attack object command - matches common C++ pattern
    pub fn create_attack_object(
        attackers: Vec<ObjectID>,
        target: ObjectID,
        player_id: Int,
    ) -> Command {
        let mut command = Command::new(CommandType::DoAttackObject);
        command.set_player_index(player_id);

        // First argument is target
        command.append_object_id_argument(target);

        // Then add all attacker IDs
        for attacker_id in attackers {
            command.append_object_id_argument(attacker_id);
        }

        command
    }

    /// Create area selection command - matches C++ AreaSelection
    pub fn create_area_selection(region: IRegion2D, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::AreaSelection);
        command.set_player_index(player_id);
        command.append_pixel_region_argument(region);
        command
    }

    /// Create build structure command
    pub fn create_build_structure(builder: ObjectID, position: Coord3D, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::DozerConstruct);
        command.set_player_index(player_id);
        command.append_object_id_argument(builder);
        command.append_location_argument(position);
        command
    }

    /// Create unit production command
    pub fn create_produce_unit(factory: ObjectID, count: Int, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::QueueUnitCreate);
        command.set_player_index(player_id);
        command.append_object_id_argument(factory);
        command.append_integer_argument(count);
        command
    }

    /// Create stop command for units
    pub fn create_stop_command(objects: Vec<ObjectID>, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::DoStop);
        command.set_player_index(player_id);

        for object_id in objects {
            command.append_object_id_argument(object_id);
        }

        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_creation() {
        let command = Command::new(CommandType::DoMoveTo);
        assert_eq!(command.get_type(), CommandType::DoMoveTo);
        assert_eq!(command.get_argument_count(), 0);
        assert!(command.id > 0);
    }

    #[test]
    fn test_command_arguments() {
        let mut command = Command::new(CommandType::DoAttackObject);
        command.append_integer_argument(42);
        command.append_real_argument(3.14);
        command.append_boolean_argument(true);

        assert_eq!(command.get_argument_count(), 3);

        if let Some(CommandArgumentType::Integer(val)) = command.get_argument(0) {
            assert_eq!(*val, 42);
        } else {
            panic!("Expected integer argument");
        }
    }

    #[test]
    fn test_command_types() {
        assert!(CommandType::DoMoveTo.is_network_message());
        assert!(!CommandType::MetaStop.is_network_message());
        assert!(CommandType::MetaStop.is_meta_message());
        assert!(CommandType::RawMouseLeftClick.is_raw_mouse_message());
    }

    #[test]
    fn test_command_builder() {
        let objects = vec![1, 2, 3];
        let position = [100.0, 200.0, 0.0];
        let command = command_builder::create_move_to_position(objects, position.into(), 1);

        assert_eq!(command.get_type(), CommandType::DoMoveTo);
        assert_eq!(command.get_player_index(), 1);
        assert_eq!(command.get_argument_count(), 4); // 1 position + 3 objects
    }

    #[test]
    fn test_command_validation() {
        let validator = DefaultCommandValidator;
        let valid_command = Command::new(CommandType::DoMoveTo);
        let invalid_command = Command::new(CommandType::Invalid);

        assert_eq!(
            validator.validate_command(&valid_command),
            CommandValidation::Valid
        );
        assert_eq!(
            validator.validate_command(&invalid_command),
            CommandValidation::InvalidType
        );
    }
}
