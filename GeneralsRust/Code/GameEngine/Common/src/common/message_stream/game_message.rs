#![allow(missing_docs)]

//! Game Message System
//!
//! This module defines the core GameMessage types and structures used
//! throughout the message stream system.

use std::fmt;

/// Invalid translator ID constant
pub const TRANSLATOR_ID_INVALID: i32 = -1;

/// Unique identifier for message stream translators
pub type TranslatorID = u32;

/// Object ID type
pub type ObjectID = u32;

/// Drawable ID type  
pub type DrawableID = u32;

/// 3D coordinate
#[derive(Debug, Clone, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Coord3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Coord3D {
    #[must_use]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// 2D integer coordinate
#[derive(Debug, Clone, PartialEq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl Default for ICoord2D {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl ICoord2D {
    #[must_use]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// 2D integer region
#[derive(Debug, Clone, PartialEq)]
pub struct IRegion2D {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Default for IRegion2D {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

/// Wide character type
pub type WideChar = char;

/// Union of possible data types for game message arguments
#[derive(Debug, Clone)]
pub enum GameMessageArgumentType {
    Integer(i32),
    Real(f32),
    Boolean(bool),
    ObjectID(ObjectID),
    DrawableID(DrawableID),
    TeamID(u32),
    SquadID(u32),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(u32),
    WideChar(WideChar),
    String(String),
}

/// Data type enum for game message arguments
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameMessageArgumentDataType {
    Integer,
    Real,
    Boolean,
    ObjectID,
    DrawableID,
    TeamID,
    Location,
    Pixel,
    PixelRegion,
    Timestamp,
    WideChar,
    String,
    Unknown,
}

impl From<&GameMessageArgumentType> for GameMessageArgumentDataType {
    fn from(arg_type: &GameMessageArgumentType) -> Self {
        match arg_type {
            GameMessageArgumentType::Integer(_) => Self::Integer,
            GameMessageArgumentType::Real(_) => Self::Real,
            GameMessageArgumentType::Boolean(_) => Self::Boolean,
            GameMessageArgumentType::ObjectID(_) => Self::ObjectID,
            GameMessageArgumentType::DrawableID(_) => Self::DrawableID,
            GameMessageArgumentType::TeamID(_) => Self::TeamID,
            GameMessageArgumentType::SquadID(_) => Self::TeamID, // Squad ID maps to Team ID
            GameMessageArgumentType::Location(_) => Self::Location,
            GameMessageArgumentType::Pixel(_) => Self::Pixel,
            GameMessageArgumentType::PixelRegion(_) => Self::PixelRegion,
            GameMessageArgumentType::Timestamp(_) => Self::Timestamp,
            GameMessageArgumentType::WideChar(_) => Self::WideChar,
            GameMessageArgumentType::String(_) => Self::String,
        }
    }
}

/// Game message argument
#[derive(Debug, Clone)]
pub struct GameMessageArgument {
    pub data: GameMessageArgumentType,
}

impl GameMessageArgument {
    pub fn new(data: GameMessageArgumentType) -> Self {
        Self { data }
    }

    pub fn get_data_type(&self) -> GameMessageArgumentDataType {
        GameMessageArgumentDataType::from(&self.data)
    }
}

/// The various messages which can be sent in a MessageStream
///
/// This enum contains all possible message types from the original C++ code,
/// organized by category for better maintainability.
#[derive(Debug, Clone, PartialEq)]
pub enum GameMessageType {
    // Basic messages
    Invalid,
    FrameTick(u32), // timestamp

    // Raw mouse messages (client to server)
    RawMousePosition(ICoord2D),
    RawMouseLeftButtonDown(ICoord2D, u32, u32), // pixel, modifiers, time
    RawMouseLeftDoubleClick(ICoord2D, u32, u32),
    RawMouseLeftButtonUp(ICoord2D, u32, u32),
    RawMouseLeftClick(ICoord2D, u32, u32),
    RawMouseLeftDrag(ICoord2D, ICoord2D), // start, end
    RawMouseMiddleButtonDown(ICoord2D, u32, u32),
    RawMouseMiddleDoubleClick(ICoord2D, u32, u32),
    RawMouseMiddleButtonUp(ICoord2D, u32, u32),
    RawMouseMiddleDrag(ICoord2D, ICoord2D),
    RawMouseRightButtonDown(ICoord2D, u32, u32),
    RawMouseRightDoubleClick(ICoord2D, u32, u32),
    RawMouseRightButtonUp(ICoord2D, u32, u32),
    RawMouseRightDrag(ICoord2D, ICoord2D),
    RawMouseWheel(i32), // spin amount

    // Keyboard messages
    RawKeyDown(u32), // key code
    RawKeyUp(u32),

    // Refined mouse messages
    MouseLeftClick(IRegion2D, u32), // region, modifiers
    MouseLeftDoubleClick(IRegion2D, u32),
    MouseMiddleClick(IRegion2D, u32),
    MouseMiddleDoubleClick(IRegion2D, u32),
    MouseRightClick(IRegion2D, u32),
    MouseRightDoubleClick(IRegion2D, u32),

    // Game control messages
    ClearGameData,
    NewGame,

    // Meta messages (virtual keystrokes for remapping)
    // View management
    MetaSaveView(u8), // view number 1-8
    MetaViewView(u8),

    // Team management
    MetaCreateTeam(u8), // team number 0-9
    MetaSelectTeam(u8),
    MetaAddTeam(u8),
    MetaViewTeam(u8),

    // Unit selection
    MetaSelectMatchingUnits,
    MetaSelectNextUnit,
    MetaSelectPrevUnit,
    MetaSelectNextWorker,
    MetaSelectPrevWorker,
    MetaSelectHero,
    MetaSelectAll,
    MetaSelectAllAircraft,

    // Unit commands
    MetaScatter,
    MetaStop,
    MetaDeploy,
    MetaCreateFormation,
    MetaFollow,

    // Communication
    MetaChatPlayers,
    MetaChatAllies,
    MetaChatEveryone,
    MetaDiplomacy,
    MetaOptions,

    // View controls
    MetaViewCommandCenter,
    MetaViewLastRadarEvent,
    MetaToggleControlBar,

    // Special modes
    MetaBeginPathBuild,
    MetaEndPathBuild,
    MetaBeginForceAttack,
    MetaEndForceAttack,
    MetaBeginForceMove,
    MetaEndForceMove,
    MetaBeginWaypoints,
    MetaEndWaypoints,
    MetaBeginPreferSelection,
    MetaEndPreferSelection,

    // Camera controls
    MetaBeginCameraRotateLeft,
    MetaEndCameraRotateLeft,
    MetaBeginCameraRotateRight,
    MetaEndCameraRotateRight,
    MetaBeginCameraZoomIn,
    MetaEndCameraZoomIn,
    MetaBeginCameraZoomOut,
    MetaEndCameraZoomOut,
    MetaCameraReset,
    MetaToggleCameraTracking,

    // Other controls
    MetaTakeScreenshot,
    MetaAllCheer,
    MetaToggleAttackMove,
    MetaToggleFastForwardReplay,
    MetaDemoInstantQuit,

    // Network messages (go over the network)
    CreateSelectedGroup(bool, Vec<ObjectID>), // create new group, object IDs
    CreateSelectedGroupNoSound(bool, Vec<ObjectID>),
    DestroySelectedGroup(u32), // team ID
    RemoveFromSelectedGroup(Vec<ObjectID>),
    SelectedGroupCommand(u32), // team ID

    CreateTeamSlot(u8), // slot 0-9
    SelectTeamSlot(u8),
    AddTeamSlot(u8),

    // Combat commands
    DoAttackSquad(Vec<ObjectID>),
    DoWeapon(u32), // weapon index
    DoWeaponAtLocation(u32, Coord3D),
    DoWeaponAtObject(u32, ObjectID),
    DoSpecialPower(u32, u32, ObjectID), // power ID, options, source
    DoSpecialPowerAtLocation(u32, Coord3D, f32, ObjectID, u32, ObjectID), // power ID, location, angle, object-in-way, options, source
    DoSpecialPowerAtObject(u32, ObjectID, u32, ObjectID), // power ID, target, options, source

    // Construction and economy
    SetRallyPoint(ObjectID, Coord3D),
    PurchaseScience(u32), // science ID
    QueueUpgrade(u32),    // upgrade ID
    CancelUpgrade(u32),
    QueueUnitCreate(u32), // unit type ID
    CancelUnitCreate(u32),
    DozerConstruct(u32, Coord3D, f32), // building type, location, angle
    DozerConstructLine(u32, Coord3D, Coord3D, f32), // building type, start, end, angle
    DozerCancelConstruct(ObjectID),
    Sell(ObjectID),

    // Unit commands
    Exit(ObjectID),
    Evacuate,
    /// C++ `MSG_EVACUATE` can carry an optional world destination for
    /// NEED_TARGET_POS evacuate commands.
    EvacuateAtLocation(Coord3D),
    ExecuteRailedTransport,
    CombatDropAtLocation(Coord3D),
    CombatDropAtObject(ObjectID),
    AreaSelection(IRegion2D),
    DoAttackObject(ObjectID), // target
    DoForceAttackObject(ObjectID),
    DoForceAttackGround(Coord3D),
    GetRepaired(ObjectID),        // repair facility
    GetHealed(ObjectID),          // heal facility
    DoRepair(ObjectID),           // target
    ResumeConstruction(ObjectID), // building
    Enter(ObjectID, ObjectID),    // selection ID, container
    Dock(ObjectID),               // dock building
    DoMoveTo(Coord3D),
    DoAttackMoveTo(Coord3D),
    DoForceMoveTO(Coord3D),
    AddWaypoint(Coord3D),
    /// Guard the given position with the current selected group.
    /// Matches C++ `MSG_DO_GUARD_POSITION` arguments: (location, GuardMode integer).
    DoGuardPosition(Coord3D, i32),
    /// Guard the given target object with the current selected group.
    /// Matches C++ `MSG_DO_GUARD_OBJECT` arguments: (objectID, GuardMode integer).
    DoGuardObject(ObjectID, i32),
    DoStop,
    DoScatter,

    // Special abilities
    InternetHack,
    DoCheer,
    ToggleOvercharge,
    SwitchWeapons(u32),
    ConvertToCarbomb(ObjectID, ObjectID),
    CaptureBuilding(ObjectID, ObjectID),
    DisableVehicleHack(ObjectID, ObjectID),
    StealCashHack(ObjectID, ObjectID),
    DisableBuildingHack(ObjectID, ObjectID),
    SnipeVehicle(ObjectID, ObjectID),
    DoSpecialPowerOverrideDestination(Coord3D, u32, ObjectID), // location, special power type, source
    /// Mimics C++ `MSG_DO_SALVAGE` which is intentionally set up to mimic `MSG_DO_MOVETO`.
    /// Carries only a destination location; the acting units are the player's current selection.
    DoSalvage(Coord3D),

    // UI and game state
    ClearInGamePopupMessage,
    PlaceBeacon(Coord3D),
    RemoveBeacon(Coord3D),
    SetBeaconText(Coord3D, String),
    SetReplayCamera(Coord3D, f32, f32), // position, rotation, zoom
    SelfDestruct(u32),                  // player ID
    CreateFormation(Vec<ObjectID>),
    LogicCRC(u32), // CRC value
    SetMineClearingDetail(u32),
    EnableRetaliationMode(u32, bool), // player ID, enabled

    // Hint messages
    MouseoverDrawableHint(DrawableID),
    MouseoverLocationHint(Coord3D),
    ValidGUICommandHint,
    InvalidGUICommandHint,
    AreaSelectionHint(IRegion2D),

    // Command hints
    DoAttackObjectHint(ObjectID),
    ImpossibleAttackHint,
    DoForceAttackObjectHint(ObjectID),
    DoForceAttackGroundHint(Coord3D),
    GetRepairedHint(ObjectID),
    GetHealedHint(ObjectID),
    DoRepairHint(ObjectID),
    ResumeConstructionHint(ObjectID),
    EnterHint(ObjectID),
    DockHint(ObjectID),
    DoMoveToHint(Coord3D),
    DoAttackMoveToHint(Coord3D),
    AddWaypointHint(Coord3D),
    HijackHint(ObjectID),
    SabotageHint(ObjectID),
    FirebombHint(ObjectID),
    ConvertToCarbombHint(ObjectID),
    CaptureBuildingHint(ObjectID),
    SnipeVehicleHint(ObjectID),
    DefectorHint(ObjectID),
    SetRallyPointHint(Coord3D),
    DoSpecialPowerOverrideDestinationHint(Coord3D),
    DoSalvageHint(Coord3D),
    DoInvalidHint,
    DoAttackObjectAfterMovingHint(ObjectID),
    HackHint(ObjectID),

    // Server to client messages
    Timestamp(u32),               // frame number
    ObjectCreated(ObjectID, u32), // object ID, type
    ObjectDestroyed(ObjectID),
    ObjectPosition(ObjectID, Coord3D),
    ObjectOrientation(ObjectID, f32), // object ID, angle
    ObjectJoinedTeam(ObjectID, u32),  // object ID, team ID
}

impl fmt::Display for GameMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A game message that lives on TheMessageStream or TheCommandList
///
/// Messages consist of a type, defining what the message is, and zero or more arguments
/// of various data types. The user of a message must know how many and what type of
/// arguments are valid for a given message type.
#[derive(Debug, Clone)]
pub struct GameMessage {
    message_type: GameMessageType,
    arguments: Vec<GameMessageArgument>,
    player_index: i32,
}

impl GameMessage {
    pub fn new(message_type: GameMessageType) -> Self {
        Self {
            message_type,
            arguments: Vec::new(),
            player_index: 0, // Default to local player
        }
    }

    pub fn with_player(message_type: GameMessageType, player_index: i32) -> Self {
        Self {
            message_type,
            arguments: Vec::new(),
            player_index,
        }
    }

    /// Get the message type
    pub fn get_type(&self) -> &GameMessageType {
        &self.message_type
    }

    /// Get the number of arguments
    pub fn get_argument_count(&self) -> usize {
        self.arguments.len()
    }

    /// Get the originating player index
    pub fn get_player_index(&self) -> i32 {
        self.player_index
    }

    /// Set the player index
    pub fn set_player_index(&mut self, player_index: i32) {
        self.player_index = player_index;
    }

    /// Get a string representation of the command type
    pub fn get_command_as_string(&self) -> String {
        format!("{}", self.message_type)
    }

    /// Get a static string representation of a command type
    pub fn get_command_type_as_string(message_type: &GameMessageType) -> String {
        format!("{}", message_type)
    }

    // Argument append methods

    pub fn append_integer_argument(&mut self, arg: i32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::Integer(
                arg,
            )));
    }

    pub fn append_real_argument(&mut self, arg: f32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::Real(arg)));
    }

    pub fn append_boolean_argument(&mut self, arg: bool) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::Boolean(
                arg,
            )));
    }

    pub fn append_drawable_id_argument(&mut self, arg: DrawableID) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentType::DrawableID(arg),
        ));
    }

    pub fn append_object_id_argument(&mut self, arg: ObjectID) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::ObjectID(
                arg,
            )));
    }

    pub fn append_team_id_argument(&mut self, arg: u32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::TeamID(
                arg,
            )));
    }

    pub fn append_location_argument(&mut self, arg: Coord3D) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::Location(
                arg,
            )));
    }

    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::Pixel(
                arg,
            )));
    }

    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentType::PixelRegion(arg),
        ));
    }

    pub fn append_wide_char_argument(&mut self, arg: WideChar) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::WideChar(
                arg,
            )));
    }

    pub fn append_string_argument(&mut self, arg: String) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentType::String(
                arg,
            )));
    }

    pub fn append_timestamp_argument(&mut self, arg: u32) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentType::Timestamp(arg),
        ));
    }

    /// Get a specific argument by index
    pub fn get_argument(&self, index: usize) -> Option<&GameMessageArgumentType> {
        self.arguments.get(index).map(|arg| &arg.data)
    }

    /// Get the data type of a specific argument
    pub fn get_argument_data_type(&self, index: usize) -> GameMessageArgumentDataType {
        self.arguments
            .get(index)
            .map(|arg| arg.get_data_type())
            .unwrap_or(GameMessageArgumentDataType::Unknown)
    }

    /// Get all arguments
    pub fn get_arguments(&self) -> &Vec<GameMessageArgument> {
        &self.arguments
    }
}

/// Helper function to build a 2D region from anchor and destination points
pub fn build_region(anchor: &ICoord2D, dest: &ICoord2D) -> IRegion2D {
    let min_x = anchor.x.min(dest.x);
    let max_x = anchor.x.max(dest.x);
    let min_y = anchor.y.min(dest.y);
    let max_y = anchor.y.max(dest.y);

    IRegion2D {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_message_creation() {
        let message = GameMessage::new(GameMessageType::Invalid);
        assert_eq!(message.get_type(), &GameMessageType::Invalid);
        assert_eq!(message.get_argument_count(), 0);
        assert_eq!(message.get_player_index(), 0);
    }

    #[test]
    fn test_game_message_with_player() {
        let message = GameMessage::with_player(GameMessageType::NewGame, 5);
        assert_eq!(message.get_type(), &GameMessageType::NewGame);
        assert_eq!(message.get_player_index(), 5);
    }

    #[test]
    fn test_argument_appending() {
        let mut message = GameMessage::new(GameMessageType::FrameTick(1234));

        // Test different argument types
        message.append_integer_argument(42);
        message.append_real_argument(3.14);
        message.append_boolean_argument(true);
        message.append_object_id_argument(123);

        assert_eq!(message.get_argument_count(), 4);

        // Test retrieving arguments
        if let Some(GameMessageArgumentType::Integer(value)) = message.get_argument(0) {
            assert_eq!(*value, 42);
        } else {
            panic!("Expected integer argument");
        }

        if let Some(GameMessageArgumentType::Real(value)) = message.get_argument(1) {
            assert_eq!(*value, 3.14);
        } else {
            panic!("Expected real argument");
        }

        assert_eq!(
            message.get_argument_data_type(0),
            GameMessageArgumentDataType::Integer
        );
        assert_eq!(
            message.get_argument_data_type(1),
            GameMessageArgumentDataType::Real
        );
    }

    #[test]
    fn test_coordinate_types() {
        let coord3d = Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let icoord2d = ICoord2D { x: 10, y: 20 };
        let iregion2d = IRegion2D {
            x: 5,
            y: 10,
            width: 100,
            height: 200,
        };

        let mut message = GameMessage::new(GameMessageType::DoMoveTo(coord3d.clone()));
        message.append_location_argument(coord3d.clone());
        message.append_pixel_argument(icoord2d);
        message.append_pixel_region_argument(iregion2d);

        if let Some(GameMessageArgumentType::Location(loc)) = message.get_argument(0) {
            assert_eq!(loc.x, 1.0);
            assert_eq!(loc.y, 2.0);
            assert_eq!(loc.z, 3.0);
        } else {
            panic!("Expected location argument");
        }
    }

    #[test]
    fn test_build_region() {
        let anchor = ICoord2D { x: 100, y: 50 };
        let dest = ICoord2D { x: 200, y: 150 };

        let region = build_region(&anchor, &dest);

        assert_eq!(region.x, 100);
        assert_eq!(region.y, 50);
        assert_eq!(region.width, 100);
        assert_eq!(region.height, 100);

        // Test with reversed coordinates
        let region2 = build_region(&dest, &anchor);
        assert_eq!(region, region2);
    }

    #[test]
    fn test_message_type_display() {
        let message_type = GameMessageType::DoMoveTo(Coord3D::default());
        let display_string = format!("{}", message_type);
        assert!(display_string.contains("DoMoveTo"));
    }

    #[test]
    fn test_argument_data_types() {
        use GameMessageArgumentDataType::*;

        let int_arg = GameMessageArgument::new(GameMessageArgumentType::Integer(42));
        assert_eq!(int_arg.get_data_type(), Integer);

        let real_arg = GameMessageArgument::new(GameMessageArgumentType::Real(3.14));
        assert_eq!(real_arg.get_data_type(), Real);

        let bool_arg = GameMessageArgument::new(GameMessageArgumentType::Boolean(true));
        assert_eq!(bool_arg.get_data_type(), Boolean);

        let obj_arg = GameMessageArgument::new(GameMessageArgumentType::ObjectID(123));
        assert_eq!(obj_arg.get_data_type(), ObjectID);
    }

    #[test]
    fn test_default_implementations() {
        let coord3d = Coord3D::default();
        assert_eq!(coord3d.x, 0.0);
        assert_eq!(coord3d.y, 0.0);
        assert_eq!(coord3d.z, 0.0);

        let icoord2d = ICoord2D::default();
        assert_eq!(icoord2d.x, 0);
        assert_eq!(icoord2d.y, 0);

        let iregion2d = IRegion2D::default();
        assert_eq!(iregion2d.x, 0);
        assert_eq!(iregion2d.y, 0);
        assert_eq!(iregion2d.width, 0);
        assert_eq!(iregion2d.height, 0);
    }
}
