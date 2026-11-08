//! Messages system - matches core GameMessage flow.
//!
//! The queue is owned by a game instance (`GameMessageQueue`). There is no
//! process-wide message store: two instances never share queued messages.
//! Legacy `TheMessageStream::append_message` / `append_message` adapters that
//! do not hold an owner drop the message (same observable black-hole as the
//! old undrained process queue) instead of writing shared mutable state.

use crate::commands::command::CommandType;
use crate::common::{Coord3D, DrawableID, ICoord2D, IRegion2D, Int, MessageType, ObjectID, UnsignedInt};
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};

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
pub const MSG_REMOVE_FROM_SELECTED_GROUP: u32 = CommandType::RemoveFromSelectedGroup as u32;
pub const MSG_AREA_SELECTION: u32 = CommandType::AreaSelection as u32;
pub const MSG_DO_MOVETO: u32 = CommandType::DoMoveTo as u32;
pub const MSG_DO_FORCE_ATTACK_GROUND: u32 = CommandType::DoForceAttackGround as u32;
pub const MSG_DO_ATTACK_OBJECT: u32 = CommandType::DoAttackObject as u32;
pub const MSG_DO_STOP: u32 = CommandType::DoStop as u32;
pub const MSG_QUEUE_UNIT_CREATE: u32 = CommandType::QueueUnitCreate as u32;
pub const MSG_DOZER_CONSTRUCT: u32 = CommandType::DozerConstruct as u32;
pub const MSG_SELL: u32 = CommandType::Sell as u32;
pub const MSG_LOGIC_CRC: u32 = 0xFFFF_FF01;

/// Instance-owned message queue. Construct one per game world / session.
#[derive(Debug, Default)]
pub struct GameMessageQueue {
    messages: VecDeque<GameMessage>,
}

impl GameMessageQueue {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn push(&mut self, message: GameMessage) {
        self.messages.push_back(message);
    }

    pub fn drain(&mut self) -> Vec<GameMessage> {
        self.messages.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Append and return a builder that commits into this queue on drop.
    ///
    /// The builder retains the exclusive queue borrow until it is committed or dropped.
    ///
    /// ```compile_fail
    /// # use gamelogic::messages::{GameMessageQueue, MSG_DO_STOP};
    /// let mut queue = GameMessageQueue::new();
    /// let builder = queue.append(MSG_DO_STOP);
    /// drop(queue);
    /// drop(builder);
    /// ```
    pub fn append(&mut self, message_type: MessageType) -> MessageBuilder<'_> {
        MessageBuilder::new_owned(message_type, self)
    }
}

/// Builder that queues the message when dropped into its owning queue.
///
/// A builder created without an owner (legacy `append_message`) drops the
/// message on commit instead of touching process-wide state.
#[derive(Debug)]
pub struct MessageBuilder<'a> {
    message: Option<GameMessage>,
    queue: Option<&'a mut GameMessageQueue>,
}

impl<'a> MessageBuilder<'a> {
    fn new_owned(message_type: MessageType, queue: &'a mut GameMessageQueue) -> Self {
        Self {
            message: Some(GameMessage::new(message_type)),
            queue: Some(queue),
        }
    }

    fn new_unowned(message_type: MessageType) -> Self {
        Self {
            message: Some(GameMessage::new(message_type)),
            queue: None,
        }
    }

    fn push_into_sink(&mut self, msg: GameMessage) {
        if let Some(queue) = &mut self.queue {
            queue.push(msg);
        } else {
            log::debug!(
                "GameMessageQueue: dropping unowned message id={}",
                msg.id
            );
        }
    }

    /// Explicitly commit the message to the queue.
    pub fn commit(mut self) {
        if let Some(msg) = self.message.take() {
            self.push_into_sink(msg);
        }
    }
}

impl Deref for MessageBuilder<'_> {
    type Target = GameMessage;

    fn deref(&self) -> &Self::Target {
        self.message
            .as_ref()
            .expect("Attempted to dereference consumed message")
    }
}

impl DerefMut for MessageBuilder<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.message
            .as_mut()
            .expect("Attempted to mutably dereference consumed message")
    }
}

impl Drop for MessageBuilder<'_> {
    fn drop(&mut self) {
        if let Some(msg) = self.message.take() {
            self.push_into_sink(msg);
        }
    }
}


/// Legacy adapter: build a message with no owning queue.
///
/// Prefer `GameMessageQueue::append`. Without an owner the message is dropped
/// on commit (no process-wide store).
pub fn append_message(message_type: MessageType) -> MessageBuilder<'static> {
    MessageBuilder::new_unowned(message_type)
}

/// Legacy no-op drain. Owned queues use `GameMessageQueue::drain`.
pub fn drain_messages() -> Vec<GameMessage> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    #[test]
    fn owned_queues_do_not_share_state() {
        let mut a = GameMessageQueue::new();
        let mut b = GameMessageQueue::new();
        a.append(MSG_DO_STOP).append_integer_argument(1);
        assert_eq!(a.len(), 1);
        assert!(b.is_empty());
        b.append(MSG_DO_MOVETO).append_integer_argument(2);
        assert_eq!(a.drain().len(), 1);
        assert_eq!(b.drain().len(), 1);
        assert!(a.is_empty());
        assert!(b.is_empty());
    }

    #[test]
    fn parallel_instances_keep_identical_independent_results() {
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));

        let mut handles = Vec::new();
        for seed in [11u32, 22u32] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let mut queue = GameMessageQueue::new();
                barrier.wait();
                for i in 0..5 {
                    let mut msg = queue.append(MSG_DO_STOP);
                    msg.append_integer_argument((seed + i) as i32);
                    msg.set_player_id(seed as Int);
                }
                {
                    let mut move_msg = queue.append(MSG_DO_MOVETO);
                    move_msg.append_integer_argument(seed as i32);
                }
                let drained = queue.drain();
                assert_eq!(drained.len(), 6);
                assert!(queue.is_empty());
                let fingerprint: Vec<(u32, i32, Int)> = drained
                    .iter()
                    .map(|m| {
                        let arg = match m.arguments.first() {
                            Some(MessageArgument::Integer(v)) => *v,
                            _ => -1,
                        };
                        (m.id, arg, m.player_id)
                    })
                    .collect();
                results.lock().unwrap().push((seed, fingerprint));
            }));
        }
        for h in handles {
            h.join().expect("thread");
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(seed, _)| *seed);
        assert_eq!(got.len(), 2);
        for (seed, fingerprint) in got.iter() {
            assert_eq!(fingerprint.len(), 6);
            for (i, (id, arg, player)) in fingerprint.iter().take(5).enumerate() {
                assert_eq!(*id, MSG_DO_STOP);
                assert_eq!(*arg, (*seed + i as u32) as i32);
                assert_eq!(*player, *seed as Int);
            }
            assert_eq!(fingerprint[5].0, MSG_DO_MOVETO);
            assert_eq!(fingerprint[5].1, *seed as i32);
        }
        assert_ne!(got[0].1, got[1].1);
    }

    #[test]
    fn unowned_append_is_dropped_not_shared() {
        let _ = append_message(MSG_DO_STOP);
        assert!(drain_messages().is_empty());
        let mut owned = GameMessageQueue::new();
        assert!(owned.drain().is_empty());
    }
}
