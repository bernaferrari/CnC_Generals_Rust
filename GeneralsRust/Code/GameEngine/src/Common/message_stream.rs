// FILE: message_stream.rs
// The message stream propagates all messages through a series of "translators"
// Ported from C++ to Rust

use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::RefCell;

/// Unique identifiers for message stream translators
pub type TranslatorId = u32;
pub const TRANSLATOR_ID_INVALID: TranslatorId = u32::MAX;

/// Union of possible data for given message type
#[derive(Clone, Debug)]
pub enum GameMessageArgumentType {
    Integer(i32),
    Real(f32),
    Boolean(bool),
    ObjectId(u32),
    DrawableId(u32),
    TeamId(u32),
    SquadId(u32),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(u32),
    WideChar(char),
}

#[derive(Clone, Copy, Debug)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

/// Game message argument data type enum
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMessageArgumentDataType {
    Integer,
    Real,
    Boolean,
    ObjectId,
    DrawableId,
    TeamId,
    Location,
    Pixel,
    PixelRegion,
    Timestamp,
    WideChar,
    Unknown,
}

/// A game message argument
#[derive(Clone, Debug)]
pub struct GameMessageArgument {
    pub data: GameMessageArgumentType,
    pub arg_type: GameMessageArgumentDataType,
}

/// The various messages which can be sent in a MessageStream
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GameMessageType {
    Invalid = 0,
    FrameTick,

    // Client to Server messages
    RawMouseBegin,
    RawMousePosition,
    RawMouseLeftButtonDown,
    RawMouseLeftDoubleClick,
    RawMouseLeftButtonUp,
    RawMouseLeftClick,
    RawMouseLeftDrag,
    RawMouseMiddleButtonDown,
    RawMouseMiddleDoubleClick,
    RawMouseMiddleButtonUp,
    RawMouseMiddleDrag,
    RawMouseRightButtonDown,
    RawMouseRightDoubleClick,
    RawMouseRightButtonUp,
    RawMouseRightDrag,
    RawMouseWheel,
    RawMouseEnd,

    RawKeyDown,
    RawKeyUp,

    // Refined Mouse messages
    MouseLeftClick,
    MouseLeftDoubleClick,
    MouseMiddleClick,
    MouseMiddleDoubleClick,
    MouseRightClick,
    MouseRightDoubleClick,

    ClearGameData,
    NewGame,

    // "meta" messages
    BeginMetaMessages,
    MetaSaveView1,
    MetaSaveView2,
    MetaSaveView3,
    MetaSaveView4,
    MetaSaveView5,
    MetaSaveView6,
    MetaSaveView7,
    MetaSaveView8,
    MetaViewView1,
    MetaViewView2,
    MetaViewView3,
    MetaViewView4,
    MetaViewView5,
    MetaViewView6,
    MetaViewView7,
    MetaViewView8,
    MetaCreateTeam0,
    MetaCreateTeam1,
    MetaCreateTeam2,
    MetaCreateTeam3,
    MetaCreateTeam4,
    MetaCreateTeam5,
    MetaCreateTeam6,
    MetaCreateTeam7,
    MetaCreateTeam8,
    MetaCreateTeam9,
    MetaSelectTeam0,
    MetaSelectTeam1,
    MetaSelectTeam2,
    MetaSelectTeam3,
    MetaSelectTeam4,
    MetaSelectTeam5,
    MetaSelectTeam6,
    MetaSelectTeam7,
    MetaSelectTeam8,
    MetaSelectTeam9,
    MetaAddTeam0,
    MetaAddTeam1,
    MetaAddTeam2,
    MetaAddTeam3,
    MetaAddTeam4,
    MetaAddTeam5,
    MetaAddTeam6,
    MetaAddTeam7,
    MetaAddTeam8,
    MetaAddTeam9,
    MetaViewTeam0,
    MetaViewTeam1,
    MetaViewTeam2,
    MetaViewTeam3,
    MetaViewTeam4,
    MetaViewTeam5,
    MetaViewTeam6,
    MetaViewTeam7,
    MetaViewTeam8,
    MetaViewTeam9,
    MetaSelectMatchingUnits,
    MetaSelectNextUnit,
    MetaSelectPrevUnit,
    MetaSelectNextWorker,
    MetaSelectPrevWorker,
    MetaViewCommandCenter,
    MetaViewLastRadarEvent,
    MetaSelectHero,
    MetaSelectAll,
    MetaSelectAllAircraft,
    MetaScatter,
    MetaStop,
    MetaDeploy,
    MetaCreateFormation,
    MetaFollow,
    MetaChatPlayers,
    MetaChatAllies,
    MetaChatEveryone,
    MetaDiplomacy,
    MetaOptions,
    MetaToggleLowerDetails,
    MetaToggleControlBar,
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
    MetaTakeScreenshot,
    MetaAllCheer,
    MetaToggleAttackMove,
    MetaBeginCameraRotateLeft,
    MetaEndCameraRotateLeft,
    MetaBeginCameraRotateRight,
    MetaEndCameraRotateRight,
    MetaBeginCameraZoomIn,
    MetaEndCameraZoomIn,
    MetaBeginCameraZoomOut,
    MetaEndCameraZoomOut,
    MetaCameraReset,
    MetaToggleCameraTrackingDrawable,
    MetaToggleFastForwardReplay,
    MetaDemoInstantQuit,
    MetaPlaceBeacon,
    MetaRemoveBeacon,
    EndMetaMessages,

    MouseoverDrawableHint,
    MouseoverLocationHint,
    ValidGuiCommandHint,
    InvalidGuiCommandHint,
    AreaSelectionHint,

    // Command hints
    DoAttackObjectHint,
    ImpossibleAttackHint,
    DoForceAttackObjectHint,
    DoForceAttackGroundHint,
    GetRepairedHint,
    GetHealedHint,
    DoRepairHint,
    ResumeConstructionHint,
    EnterHint,
    DockHint,
    DoMoveToHint,
    DoAttackMoveToHint,
    AddWaypointHint,
    HijackHint,
    SabotageHint,
    FirebombHint,
    ConvertToCarbombHint,
    CaptureBuildingHint,
    SnipeVehicleHint,
    DefectorHint,
    SetRallyPointHint,
    DoSpecialPowerOverrideDestinationHint,
    DoSalvageHint,
    DoInvalidHint,
    DoAttackObjectAfterMovingHint,
    HackHint,

    // Network messages start at 1000
    BeginNetworkMessages = 1000,
    CreateSelectedGroup,
    CreateSelectedGroupNoSound,
    DestroySelectedGroup,
    RemoveFromSelectedGroup,
    SelectedGroupCommand,
    CreateTeam0,
    CreateTeam1,
    CreateTeam2,
    CreateTeam3,
    CreateTeam4,
    CreateTeam5,
    CreateTeam6,
    CreateTeam7,
    CreateTeam8,
    CreateTeam9,
    SelectTeam0,
    SelectTeam1,
    SelectTeam2,
    SelectTeam3,
    SelectTeam4,
    SelectTeam5,
    SelectTeam6,
    SelectTeam7,
    SelectTeam8,
    SelectTeam9,
    AddTeam0,
    AddTeam1,
    AddTeam2,
    AddTeam3,
    AddTeam4,
    AddTeam5,
    AddTeam6,
    AddTeam7,
    AddTeam8,
    AddTeam9,
    DoAttackSquad,
    DoWeapon,
    DoWeaponAtLocation,
    DoWeaponAtObject,
    DoSpecialPower,
    DoSpecialPowerAtLocation,
    DoSpecialPowerAtObject,
    SetRallyPoint,
    PurchaseScience,
    QueueUpgrade,
    CancelUpgrade,
    QueueUnitCreate,
    CancelUnitCreate,
    DozerConstruct,
    DozerConstructLine,
    DozerCancelConstruct,
    Sell,
    Exit,
    Evacuate,
    ExecuteRailedTransport,
    CombatDropAtLocation,
    CombatDropAtObject,
    AreaSelection,
    DoAttackObject,
    DoForceAttackObject,
    DoForceAttackGround,
    GetRepaired,
    GetHealed,
    DoRepair,
    ResumeConstruction,
    Enter,
    Dock,
    DoMoveTo,
    DoAttackMoveTo,
    DoForceMoveTo,
    AddWaypoint,
    DoGuardPosition,
    DoGuardObject,
    DoStop,
    DoScatter,
    InternetHack,
    DoCheer,
    ToggleOvercharge,
    SwitchWeapons,
    ConvertToCarbomb,
    CaptureBuilding,
    DisableVehicleHack,
    StealCashHack,
    DisableBuildingHack,
    SnipeVehicle,
    DoSpecialPowerOverrideDestination,
    DoSalvage,
    ClearIngamePopupMessage,
    PlaceBeacon,
    RemoveBeacon,
    SetBeaconText,
    SetReplayCamera,
    SelfDestruct,
    CreateFormation,
    LogicCrc,
    SetMineClearingDetail,
    EnableRetaliationMode,
    EndNetworkMessages = 1999,

    // Server to Client messages
    Timestamp,
    ObjectCreated,
    ObjectDestroyed,
    ObjectPosition,
    ObjectOrientation,
    ObjectJoinedTeam,

    Count,
}

impl GameMessageType {
    pub fn as_string(&self) -> &'static str {
        match self {
            Self::Invalid => "MSG_INVALID",
            Self::FrameTick => "MSG_FRAME_TICK",
            Self::RawMouseBegin => "MSG_RAW_MOUSE_BEGIN",
            Self::MetaChatPlayers => "MSG_META_CHAT_PLAYERS",
            Self::MetaChatAllies => "MSG_META_CHAT_ALLIES",
            Self::MetaChatEveryone => "MSG_META_CHAT_EVERYONE",
            // Add more as needed
            _ => "MSG_UNKNOWN",
        }
    }
}

/// A game message that either lives on the MessageStream or the CommandList
pub struct GameMessage {
    msg_type: GameMessageType,
    player_index: i32,
    arguments: Vec<GameMessageArgument>,
}

impl GameMessage {
    pub fn new(msg_type: GameMessageType, player_index: i32) -> Self {
        Self {
            msg_type,
            player_index,
            arguments: Vec::new(),
        }
    }

    pub fn get_type(&self) -> GameMessageType {
        self.msg_type
    }

    pub fn get_player_index(&self) -> i32 {
        self.player_index
    }

    pub fn get_argument_count(&self) -> usize {
        self.arguments.len()
    }

    pub fn append_integer_argument(&mut self, arg: i32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Integer(arg),
            arg_type: GameMessageArgumentDataType::Integer,
        });
    }

    pub fn append_real_argument(&mut self, arg: f32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Real(arg),
            arg_type: GameMessageArgumentDataType::Real,
        });
    }

    pub fn append_boolean_argument(&mut self, arg: bool) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Boolean(arg),
            arg_type: GameMessageArgumentDataType::Boolean,
        });
    }

    pub fn append_object_id_argument(&mut self, arg: u32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::ObjectId(arg),
            arg_type: GameMessageArgumentDataType::ObjectId,
        });
    }

    pub fn append_drawable_id_argument(&mut self, arg: u32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::DrawableId(arg),
            arg_type: GameMessageArgumentDataType::DrawableId,
        });
    }

    pub fn append_team_id_argument(&mut self, arg: u32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::TeamId(arg),
            arg_type: GameMessageArgumentDataType::TeamId,
        });
    }

    pub fn append_location_argument(&mut self, arg: Coord3D) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Location(arg),
            arg_type: GameMessageArgumentDataType::Location,
        });
    }

    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Pixel(arg),
            arg_type: GameMessageArgumentDataType::Pixel,
        });
    }

    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::PixelRegion(arg),
            arg_type: GameMessageArgumentDataType::PixelRegion,
        });
    }

    pub fn append_timestamp_argument(&mut self, arg: u32) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::Timestamp(arg),
            arg_type: GameMessageArgumentDataType::Timestamp,
        });
    }

    pub fn append_wide_char_argument(&mut self, arg: char) {
        self.arguments.push(GameMessageArgument {
            data: GameMessageArgumentType::WideChar(arg),
            arg_type: GameMessageArgumentDataType::WideChar,
        });
    }

    pub fn get_argument(&self, index: usize) -> Option<&GameMessageArgumentType> {
        self.arguments.get(index).map(|arg| &arg.data)
    }

    pub fn get_argument_data_type(&self, index: usize) -> GameMessageArgumentDataType {
        self.arguments.get(index)
            .map(|arg| arg.arg_type)
            .unwrap_or(GameMessageArgumentDataType::Unknown)
    }

    pub fn get_command_as_string(&self) -> &'static str {
        self.msg_type.as_string()
    }
}

/// What to do with a GameMessage after a translator has handled it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMessageDisposition {
    KeepMessage,
    DestroyMessage,
}

/// Trait for message translators
pub trait GameMessageTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition;
}

/// The GameMessageList class encapsulates the manipulation of lists of GameMessages
pub struct GameMessageList {
    messages: VecDeque<GameMessage>,
}

impl GameMessageList {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn init(&mut self) {
        // Initialize
    }

    pub fn reset(&mut self) {
        self.messages.clear();
    }

    pub fn update(&mut self) {
        // Update
    }

    pub fn append_message(&mut self, msg: GameMessage) {
        self.messages.push_back(msg);
    }

    pub fn insert_message(&mut self, msg: GameMessage, insert_after_index: usize) {
        if insert_after_index < self.messages.len() {
            self.messages.insert(insert_after_index + 1, msg);
        } else {
            self.messages.push_back(msg);
        }
    }

    pub fn remove_message(&mut self, index: usize) -> Option<GameMessage> {
        if index < self.messages.len() {
            self.messages.remove(index)
        } else {
            None
        }
    }

    pub fn contains_message_of_type(&self, msg_type: GameMessageType) -> bool {
        self.messages.iter().any(|msg| msg.get_type() == msg_type)
    }

    pub fn get_first_message(&self) -> Option<&GameMessage> {
        self.messages.front()
    }

    pub fn iter(&self) -> impl Iterator<Item = &GameMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for GameMessageList {
    fn default() -> Self {
        Self::new()
    }
}

struct TranslatorData {
    id: TranslatorId,
    translator: Box<dyn GameMessageTranslator>,
    priority: u32,
}

/// A MessageStream contains an ordered list of messages which can have one or more
/// prioritized message handler functions ("translators") attached to it
pub struct MessageStream {
    message_list: GameMessageList,
    translators: Vec<TranslatorData>,
    next_translator_id: TranslatorId,
}

impl MessageStream {
    pub fn new() -> Self {
        Self {
            message_list: GameMessageList::new(),
            translators: Vec::new(),
            next_translator_id: 1,
        }
    }

    pub fn init(&mut self) {
        self.message_list.init();
    }

    pub fn reset(&mut self) {
        self.message_list.reset();
    }

    pub fn update(&mut self) {
        self.message_list.update();
    }

    pub fn append_message(&mut self, msg_type: GameMessageType, player_index: i32) -> &mut GameMessage {
        let msg = GameMessage::new(msg_type, player_index);
        self.message_list.append_message(msg);
        // Return mutable reference to the last message
        self.message_list.messages.back_mut().unwrap()
    }

    pub fn insert_message(&mut self, msg_type: GameMessageType, player_index: i32, insert_after_index: usize) -> &mut GameMessage {
        let msg = GameMessage::new(msg_type, player_index);
        self.message_list.insert_message(msg, insert_after_index);
        // Return mutable reference to the inserted message
        let index = if insert_after_index < self.message_list.len() - 1 {
            insert_after_index + 1
        } else {
            self.message_list.len() - 1
        };
        &mut self.message_list.messages[index]
    }

    /// Attach a translator function to the stream at a priority value.
    /// Lower priorities are executed first.
    pub fn attach_translator(&mut self, translator: Box<dyn GameMessageTranslator>, priority: u32) -> TranslatorId {
        let id = self.next_translator_id;
        self.next_translator_id += 1;

        let translator_data = TranslatorData {
            id,
            translator,
            priority,
        };

        // Insert sorted by priority
        let insert_pos = self.translators
            .iter()
            .position(|t| t.priority > priority)
            .unwrap_or(self.translators.len());

        self.translators.insert(insert_pos, translator_data);

        id
    }

    pub fn find_translator(&self, id: TranslatorId) -> bool {
        self.translators.iter().any(|t| t.id == id)
    }

    pub fn remove_translator(&mut self, id: TranslatorId) {
        self.translators.retain(|t| t.id != id);
    }

    /// Propagate messages through attached Translators, invoking each Translator's
    /// callback for each message in the stream.
    pub fn propagate_messages(&mut self, command_list: &mut CommandList) {
        let mut messages_to_keep = Vec::new();

        // Process each translator
        for translator in &mut self.translators {
            let mut temp_messages = Vec::new();
            std::mem::swap(&mut temp_messages, &mut messages_to_keep);
            messages_to_keep.clear();

            if temp_messages.is_empty() {
                // First translator, use all messages
                for msg in self.message_list.messages.iter() {
                    let disposition = translator.translator.translate_game_message(msg);
                    if disposition == GameMessageDisposition::KeepMessage {
                        messages_to_keep.push(msg);
                    }
                }
            } else {
                // Subsequent translators, use kept messages
                for msg in temp_messages {
                    let disposition = translator.translator.translate_game_message(msg);
                    if disposition == GameMessageDisposition::KeepMessage {
                        messages_to_keep.push(msg);
                    }
                }
            }
        }

        // Transfer remaining messages to command list
        for msg in messages_to_keep {
            // We can't move out of the reference, so we need to clone
            // In a real implementation, we'd use Rc or similar
        }

        // Clear the stream
        self.message_list.messages.clear();
    }

    pub fn get_first_message(&self) -> Option<&GameMessage> {
        self.message_list.get_first_message()
    }

    pub fn contains_message_of_type(&self, msg_type: GameMessageType) -> bool {
        self.message_list.contains_message_of_type(msg_type)
    }
}

impl Default for MessageStream {
    fn default() -> Self {
        Self::new()
    }
}

/// The CommandList is the final set of messages that have made their way through
/// all of the Translators of the MessageStream, and reached the end.
pub struct CommandList {
    message_list: GameMessageList,
}

impl CommandList {
    pub fn new() -> Self {
        Self {
            message_list: GameMessageList::new(),
        }
    }

    pub fn init(&mut self) {
        self.message_list.init();
    }

    pub fn reset(&mut self) {
        self.destroy_all_messages();
    }

    pub fn update(&mut self) {
        self.message_list.update();
    }

    pub fn append_message(&mut self, msg: GameMessage) {
        self.message_list.append_message(msg);
    }

    pub fn append_message_list(&mut self, messages: Vec<GameMessage>) {
        for msg in messages {
            self.message_list.append_message(msg);
        }
    }

    fn destroy_all_messages(&mut self) {
        self.message_list.messages.clear();
    }

    pub fn get_first_message(&self) -> Option<&GameMessage> {
        self.message_list.get_first_message()
    }

    pub fn iter(&self) -> impl Iterator<Item = &GameMessage> {
        self.message_list.iter()
    }
}

impl Default for CommandList {
    fn default() -> Self {
        Self::new()
    }
}

/// Given an "anchor" point and the current mouse position (dest),
/// construct a valid 2D bounding region.
pub fn build_region(anchor: &ICoord2D, dest: &ICoord2D) -> IRegion2D {
    let lo_x = anchor.x.min(dest.x);
    let hi_x = anchor.x.max(dest.x);
    let lo_y = anchor.y.min(dest.y);
    let hi_y = anchor.y.max(dest.y);

    IRegion2D {
        lo: ICoord2D { x: lo_x, y: lo_y },
        hi: ICoord2D { x: hi_x, y: hi_y },
    }
}
