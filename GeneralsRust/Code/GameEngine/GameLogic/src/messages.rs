//! Messages system - matches core GameMessage flow.

use crate::commands::command::CommandType;
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, ICoord2D, IRegion2D, Int, MessageType, ObjectID,
    UnsignedInt,
};
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

/// Game message structure
#[derive(Debug, Clone)]
pub struct GameMessage {
    pub id: u32,
    pub content: String,
    pub arguments: Vec<MessageArgument>,
    pub player_id: Int,
}

#[derive(Debug, Clone)]
pub enum MessageArgument {
    Boolean(bool),
    Integer(i32),
    ObjectId(ObjectID),
    DrawableId(DrawableID),
    TeamId(UnsignedInt),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(UnsignedInt),
    String(String),
}

impl GameMessage {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            content: String::new(),
            arguments: Vec::new(),
            player_id: 0,
        }
    }

    pub fn append_boolean_argument(&mut self, arg: bool) {
        self.arguments.push(MessageArgument::Boolean(arg));
    }

    pub fn append_object_id_argument(&mut self, arg: ObjectID) {
        self.arguments.push(MessageArgument::ObjectId(arg));
    }

    pub fn append_drawable_id_argument(&mut self, arg: DrawableID) {
        self.arguments.push(MessageArgument::DrawableId(arg));
    }

    pub fn append_integer_argument(&mut self, arg: i32) {
        self.arguments.push(MessageArgument::Integer(arg));
    }

    pub fn append_team_id_argument(&mut self, arg: UnsignedInt) {
        self.arguments.push(MessageArgument::TeamId(arg));
    }

    pub fn append_location_argument(&mut self, arg: Coord3D) {
        self.arguments.push(MessageArgument::Location(arg));
    }

    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        self.arguments.push(MessageArgument::Pixel(arg));
    }

    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        self.arguments.push(MessageArgument::PixelRegion(arg));
    }

    pub fn append_timestamp_argument(&mut self, arg: UnsignedInt) {
        self.arguments.push(MessageArgument::Timestamp(arg));
    }

    pub fn append_string_argument(&mut self, arg: String) {
        self.arguments.push(MessageArgument::String(arg));
    }

    pub fn set_player_id(&mut self, player_id: Int) {
        self.player_id = player_id;
    }

    pub fn player_id(&self) -> Int {
        self.player_id
    }
}

// Message constants
pub const MSG_CREATE_SELECTED_GROUP: u32 = CommandType::CreateSelectedGroup as u32;
pub const MSG_CREATE_SELECTED_GROUP_NO_SOUND: u32 = CommandType::CreateSelectedGroupNoSound as u32;
pub const MSG_AREA_SELECTION: u32 = CommandType::AreaSelection as u32;
pub const MSG_DO_MOVETO: u32 = CommandType::DoMoveTo as u32;
pub const MSG_DO_FORCE_ATTACK_GROUND: u32 = CommandType::DoForceAttackGround as u32;
pub const MSG_DO_ATTACK_OBJECT: u32 = CommandType::DoAttackObject as u32;
pub const MSG_DO_STOP: u32 = CommandType::DoStop as u32;
pub const MSG_QUEUE_UNIT_CREATE: u32 = CommandType::QueueUnitCreate as u32;
pub const MSG_DOZER_CONSTRUCT: u32 = CommandType::DozerConstruct as u32;
pub const MSG_SELL: u32 = CommandType::Sell as u32;
pub const MSG_LOGIC_CRC: u32 = 0xFFFF_FF01;

static MESSAGE_QUEUE: Lazy<Mutex<VecDeque<GameMessage>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));

/// Builder that queues the message when dropped.
#[derive(Debug)]
pub struct MessageBuilder {
    message: Option<GameMessage>,
}

impl MessageBuilder {
    fn new(message_type: MessageType) -> Self {
        Self {
            message: Some(GameMessage::new(message_type)),
        }
    }

    /// Explicitly commit the message to the queue.
    pub fn commit(mut self) {
        if let Some(msg) = self.message.take() {
            MESSAGE_QUEUE
                .lock()
                .expect("Message queue poisoned")
                .push_back(msg);
        }
    }
}

impl Deref for MessageBuilder {
    type Target = GameMessage;

    fn deref(&self) -> &Self::Target {
        self.message
            .as_ref()
            .expect("Attempted to dereference consumed message")
    }
}

impl DerefMut for MessageBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.message
            .as_mut()
            .expect("Attempted to mutably dereference consumed message")
    }
}

impl Drop for MessageBuilder {
    fn drop(&mut self) {
        if let Some(msg) = self.message.take() {
            MESSAGE_QUEUE
                .lock()
                .expect("Message queue poisoned")
                .push_back(msg);
        }
    }
}

/// Append a message and return a builder for argument construction.
pub fn append_message(message_type: MessageType) -> MessageBuilder {
    MessageBuilder::new(message_type)
}

/// Drain all pending messages.
pub fn drain_messages() -> Vec<GameMessage> {
    let mut queue = MESSAGE_QUEUE.lock().expect("Message queue poisoned");
    queue.drain(..).collect()
}
