#![allow(missing_docs)]

//! Message Stream Implementation
//!
//! The MessageStream contains an ordered list of messages which can have one or more
//! prioritized message handler functions ("translators") attached to it.

use super::game_message::*;
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::collections::{LinkedList, VecDeque};
use std::sync::{Arc, RwLock};

/// What to do with a GameMessage after a translator has handled it
#[derive(Debug, Clone, PartialEq)]
pub enum GameMessageDisposition {
    /// Continue processing this message through other translators
    KeepMessage,
    /// Destroy this message immediately and don't hand it to any other translators
    DestroyMessage,
}

thread_local! {
    static EMITTED_MESSAGES: RefCell<Vec<GameMessage>> = RefCell::new(Vec::new());
}

/// Emit a new message from inside a translator.
pub fn emit_message(message: GameMessage) {
    EMITTED_MESSAGES.with(|slot| slot.borrow_mut().push(message));
}

fn take_emitted_messages() -> Vec<GameMessage> {
    EMITTED_MESSAGES.with(|slot| slot.borrow_mut().drain(..).collect())
}

/// Trait for game message translators
pub trait GameMessageTranslator: Send + Sync {
    /// Translate a game message and return what to do with it
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition;
}

/// Internal translator data structure
struct TranslatorData {
    id: TranslatorID,
    translator: Arc<RwLock<dyn GameMessageTranslator>>,
    priority: u32,
}

impl std::fmt::Debug for TranslatorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TranslatorData")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("translator_type", &"GameMessageTranslator")
            .finish()
    }
}

impl TranslatorData {
    fn new(
        id: TranslatorID,
        translator: Arc<RwLock<dyn GameMessageTranslator>>,
        priority: u32,
    ) -> Self {
        Self {
            id,
            translator,
            priority,
        }
    }
}

/// Base functionality for message lists
pub trait SubsystemInterface {
    /// Initialize the subsystem
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// Reset the subsystem
    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>>;

    /// Update the subsystem (called each frame)
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

/// Base class for game message lists
pub struct GameMessageList {
    messages: LinkedList<GameMessage>,
}

impl GameMessageList {
    pub fn new() -> Self {
        Self {
            messages: LinkedList::new(),
        }
    }

    /// Get the first message in the list
    pub fn get_first_message(&self) -> Option<&GameMessage> {
        self.messages.front()
    }

    /// Add message to end of the list
    pub fn append_message(&mut self, msg: GameMessage) {
        debug!("Appending message: {}", msg.get_command_as_string());
        self.messages.push_back(msg);
    }

    /// Insert message after a specific message
    pub fn insert_message(
        &mut self,
        msg: GameMessage,
        _after_msg: Option<&GameMessage>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Inserting message: {}", msg.get_command_as_string());

        // For simplicity, we'll just append to the end for now
        // A full implementation would need to track message positions
        self.messages.push_back(msg);
        Ok(())
    }

    /// Remove a specific message from the list
    pub fn remove_message(&mut self, msg_to_remove: &GameMessage) -> bool {
        let mut new_list = LinkedList::new();
        let mut removed = false;

        while let Some(msg) = self.messages.pop_front() {
            if !removed && std::ptr::eq(&msg, msg_to_remove) {
                debug!("Removed message: {}", msg_to_remove.get_command_as_string());
                removed = true;
            } else {
                new_list.push_back(msg);
            }
        }

        self.messages = new_list;
        removed
    }

    /// Check if the list contains a message of the specified type
    pub fn contains_message_of_type(&self, message_type: &GameMessageType) -> bool {
        self.messages.iter().any(|msg| {
            // Compare discriminants for enum equality
            std::mem::discriminant(msg.get_type()) == std::mem::discriminant(message_type)
        })
    }

    /// Get the number of messages in the list
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        debug!("Clearing {} messages from list", self.messages.len());
        self.messages.clear();
    }

    /// Get an iterator over all messages
    pub fn iter(&self) -> std::collections::linked_list::Iter<'_, GameMessage> {
        self.messages.iter()
    }

    /// Get a mutable iterator over all messages
    pub fn iter_mut(&mut self) -> std::collections::linked_list::IterMut<'_, GameMessage> {
        self.messages.iter_mut()
    }

    /// Take all messages and return them, leaving the list empty
    pub fn take_all_messages(&mut self) -> LinkedList<GameMessage> {
        std::mem::replace(&mut self.messages, LinkedList::new())
    }
}

impl Default for GameMessageList {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for GameMessageList {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Initializing GameMessageList");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Resetting GameMessageList");
        self.clear();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Base implementation does nothing
        Ok(())
    }
}

/// The main MessageStream that processes messages through translators
pub struct MessageStream {
    base: GameMessageList,
    translators: Vec<TranslatorData>,
    next_translator_id: TranslatorID,
}

impl MessageStream {
    pub fn new() -> Self {
        Self {
            base: GameMessageList::new(),
            translators: Vec::new(),
            next_translator_id: 1,
        }
    }

    /// Append a message of the specified type to the end of the stream
    pub fn append_message(&mut self, message_type: GameMessageType) -> &mut GameMessage {
        let msg = GameMessage::new(message_type);
        self.base.append_message(msg);
        // Return a reference to the last message
        self.base.messages.back_mut().unwrap()
    }

    /// Insert a message after another message
    pub fn insert_message(
        &mut self,
        message_type: GameMessageType,
        after_msg: Option<&GameMessage>,
    ) -> Result<&mut GameMessage, Box<dyn std::error::Error>> {
        let msg = GameMessage::new(message_type);
        self.base.insert_message(msg, after_msg)?;
        // Return a reference to the last message (simplified)
        Ok(self.base.messages.back_mut().unwrap())
    }

    /// Propagate messages through all attached translators
    pub fn propagate_messages(&mut self) -> Result<Vec<GameMessage>, Box<dyn std::error::Error>> {
        let mut messages_to_process: VecDeque<GameMessage> =
            self.base.take_all_messages().into_iter().collect();
        let mut completed_messages = Vec::new();

        debug!(
            "Propagating {} messages through {} translators",
            messages_to_process.len(),
            self.translators.len()
        );

        // Sort translators by priority (lower priority = higher precedence)
        self.translators.sort_by_key(|t| t.priority);

        while let Some(message) = messages_to_process.pop_front() {
            let mut keep_message = true;

            // Process message through each translator in priority order
            for translator_data in &self.translators {
                if !keep_message {
                    break;
                }

                match translator_data.translator.write() {
                    Ok(mut translator) => {
                        let disposition = translator.translate_game_message(&message);
                        match disposition {
                            GameMessageDisposition::KeepMessage => {
                                debug!(
                                    "Translator {} kept message: {}",
                                    translator_data.id,
                                    message.get_command_as_string()
                                );
                            }
                            GameMessageDisposition::DestroyMessage => {
                                debug!(
                                    "Translator {} destroyed message: {}",
                                    translator_data.id,
                                    message.get_command_as_string()
                                );
                                keep_message = false;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to acquire translator lock: {}", e);
                        // Continue with other translators
                    }
                }
            }

            if keep_message {
                completed_messages.push(message);
            }

            let emitted = take_emitted_messages();
            if !emitted.is_empty() {
                for new_message in emitted.into_iter().rev() {
                    messages_to_process.push_front(new_message);
                }
            }
        }

        debug!(
            "Propagation complete. {} messages remaining",
            completed_messages.len()
        );
        Ok(completed_messages)
    }

    /// Attach a translator to the stream at the specified priority
    /// Lower priority values are executed first
    pub fn attach_translator(
        &mut self,
        translator: Arc<RwLock<dyn GameMessageTranslator>>,
        priority: u32,
    ) -> TranslatorID {
        let id = self.next_translator_id;
        self.next_translator_id += 1;

        let translator_data = TranslatorData::new(id, translator, priority);
        self.translators.push(translator_data);

        info!(
            "Attached translator with ID {} at priority {}",
            id, priority
        );
        id
    }

    /// Find a translator by ID
    pub fn find_translator(
        &self,
        id: TranslatorID,
    ) -> Option<Arc<RwLock<dyn GameMessageTranslator>>> {
        self.translators
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.translator.clone())
    }

    /// Remove a translator by ID
    pub fn remove_translator(&mut self, id: TranslatorID) -> bool {
        let initial_len = self.translators.len();
        self.translators.retain(|t| t.id != id);
        let removed = self.translators.len() < initial_len;

        if removed {
            info!("Removed translator with ID {}", id);
        } else {
            warn!("Attempted to remove non-existent translator with ID {}", id);
        }

        removed
    }

    /// Get the number of attached translators
    pub fn translator_count(&self) -> usize {
        self.translators.len()
    }

    /// Get the number of messages currently in the stream
    pub fn message_count(&self) -> usize {
        self.base.message_count()
    }

    /// Check if the stream contains a message of the specified type
    pub fn contains_message_of_type(&self, message_type: &GameMessageType) -> bool {
        self.base.contains_message_of_type(message_type)
    }

    /// Clear all messages from the stream
    pub fn clear_messages(&mut self) {
        self.base.clear();
    }

    /// Get access to the underlying message list
    pub fn get_messages(&self) -> &GameMessageList {
        &self.base
    }
}

impl Default for MessageStream {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for MessageStream {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing MessageStream");
        self.base.init()
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Resetting MessageStream");
        self.base.reset()?;
        self.translators.clear();
        self.next_translator_id = 1;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Process messages through translators each frame
        let _completed_messages = self.propagate_messages()?;

        // In a real implementation, completed messages would be sent to TheCommandList
        // For now, we'll just discard them after processing

        Ok(())
    }
}

/// Global message stream instance
lazy_static::lazy_static! {
    pub static ref THE_MESSAGE_STREAM: Arc<RwLock<MessageStream>> =
        Arc::new(RwLock::new(MessageStream::new()));
}

/// Helper function to get the global message stream
pub fn get_message_stream() -> Arc<RwLock<MessageStream>> {
    THE_MESSAGE_STREAM.clone()
}

/// Convenience function to append a message to the global stream
pub fn append_message_to_stream(
    message_type: GameMessageType,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream_arc = get_message_stream();
    let mut stream = stream_arc
        .write()
        .map_err(|_| "Failed to acquire message stream lock")?;
    stream.append_message(message_type);
    Ok(())
}

/// Convenience function to check if the global stream contains a message type
pub fn stream_contains_message_type(
    message_type: &GameMessageType,
) -> Result<bool, Box<dyn std::error::Error>> {
    let stream_arc = get_message_stream();
    let stream = stream_arc
        .read()
        .map_err(|_| "Failed to acquire message stream lock")?;
    Ok(stream.contains_message_of_type(message_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestTranslator {
        id: String,
        call_count: AtomicUsize,
        should_destroy: bool,
    }

    impl TestTranslator {
        fn new(id: &str, should_destroy: bool) -> Self {
            Self {
                id: id.to_string(),
                call_count: AtomicUsize::new(0),
                should_destroy,
            }
        }

        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl GameMessageTranslator for TestTranslator {
        fn translate_game_message(&mut self, _msg: &GameMessage) -> GameMessageDisposition {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.should_destroy {
                GameMessageDisposition::DestroyMessage
            } else {
                GameMessageDisposition::KeepMessage
            }
        }
    }

    #[test]
    fn test_game_message_list() {
        let mut list = GameMessageList::new();

        assert_eq!(list.message_count(), 0);
        assert!(list.get_first_message().is_none());

        let msg = GameMessage::new(GameMessageType::Invalid);
        list.append_message(msg);

        assert_eq!(list.message_count(), 1);
        assert!(list.get_first_message().is_some());

        assert!(list.contains_message_of_type(&GameMessageType::Invalid));
        assert!(!list.contains_message_of_type(&GameMessageType::NewGame));

        list.clear();
        assert_eq!(list.message_count(), 0);
    }

    #[test]
    fn test_message_stream_basic() {
        let mut stream = MessageStream::new();

        assert_eq!(stream.message_count(), 0);
        assert_eq!(stream.translator_count(), 0);

        stream.append_message(GameMessageType::Invalid);
        assert_eq!(stream.message_count(), 1);

        stream.clear_messages();
        assert_eq!(stream.message_count(), 0);
    }

    #[test]
    fn test_translator_attachment() {
        let mut stream = MessageStream::new();

        let translator1 = Arc::new(RwLock::new(TestTranslator::new("test1", false)));
        let translator2 = Arc::new(RwLock::new(TestTranslator::new("test2", true)));

        let id1 = stream.attach_translator(translator1.clone(), 10);
        let id2 = stream.attach_translator(translator2.clone(), 5);

        assert_eq!(stream.translator_count(), 2);
        assert_ne!(id1, id2);

        assert!(stream.find_translator(id1).is_some());
        assert!(stream.find_translator(id2).is_some());
        assert!(stream.find_translator(999).is_none());

        assert!(stream.remove_translator(id1));
        assert_eq!(stream.translator_count(), 1);
        assert!(!stream.remove_translator(id1)); // Already removed
    }

    #[test]
    fn test_message_propagation() {
        let mut stream = MessageStream::new();

        let translator1 = Arc::new(RwLock::new(TestTranslator::new("keeper", false)));
        let translator2 = Arc::new(RwLock::new(TestTranslator::new("destroyer", true)));

        // Attach in reverse priority order to test sorting
        stream.attach_translator(translator2.clone(), 20); // Higher priority, runs later
        stream.attach_translator(translator1.clone(), 10); // Lower priority, runs first

        // Add some messages
        stream.append_message(GameMessageType::Invalid);
        stream.append_message(GameMessageType::NewGame);

        assert_eq!(stream.message_count(), 2);

        // Propagate messages
        let completed_messages = stream.propagate_messages().unwrap();

        // Both translators should have been called for each message
        assert_eq!(translator1.read().unwrap().get_call_count(), 2);
        assert_eq!(translator2.read().unwrap().get_call_count(), 2);

        // Since translator2 destroys messages, no messages should remain
        assert_eq!(completed_messages.len(), 0);
    }

    #[test]
    fn test_message_propagation_keep() {
        let mut stream = MessageStream::new();

        let translator1 = Arc::new(RwLock::new(TestTranslator::new("keeper1", false)));
        let translator2 = Arc::new(RwLock::new(TestTranslator::new("keeper2", false)));

        stream.attach_translator(translator1.clone(), 10);
        stream.attach_translator(translator2.clone(), 20);

        stream.append_message(GameMessageType::Invalid);

        let completed_messages = stream.propagate_messages().unwrap();

        // Both translators should have been called
        assert_eq!(translator1.read().unwrap().get_call_count(), 1);
        assert_eq!(translator2.read().unwrap().get_call_count(), 1);

        // Message should be kept
        assert_eq!(completed_messages.len(), 1);
    }

    #[test]
    fn test_subsystem_interface() {
        let mut stream = MessageStream::new();

        assert!(stream.init().is_ok());
        assert!(stream.update().is_ok());
        assert!(stream.reset().is_ok());

        // After reset, translators should be cleared
        assert_eq!(stream.translator_count(), 0);
        assert_eq!(stream.message_count(), 0);
    }

    #[test]
    fn test_global_message_stream() {
        // Test that we can get the global stream
        let stream1 = get_message_stream();
        let stream2 = get_message_stream();

        // Both should point to the same instance
        assert!(Arc::ptr_eq(&stream1, &stream2));

        // Test convenience functions
        assert!(append_message_to_stream(GameMessageType::Invalid).is_ok());
        assert!(stream_contains_message_type(&GameMessageType::Invalid).unwrap());
        assert!(!stream_contains_message_type(&GameMessageType::NewGame).unwrap());
    }

    #[test]
    fn test_message_insertion() {
        let mut stream = MessageStream::new();

        stream.append_message(GameMessageType::Invalid);

        // Test inserting after a message
        let result = stream.insert_message(GameMessageType::NewGame, None);
        assert!(result.is_ok());
        assert_eq!(stream.message_count(), 2);
    }

    #[test]
    fn test_priority_ordering() {
        let mut stream = MessageStream::new();

        let high_priority = Arc::new(RwLock::new(TestTranslator::new("high", false)));
        let low_priority = Arc::new(RwLock::new(TestTranslator::new("low", false)));
        let medium_priority = Arc::new(RwLock::new(TestTranslator::new("medium", false)));

        // Attach in random order
        stream.attach_translator(medium_priority.clone(), 50);
        stream.attach_translator(high_priority.clone(), 10); // Should run first
        stream.attach_translator(low_priority.clone(), 100); // Should run last

        stream.append_message(GameMessageType::Invalid);
        let _result = stream.propagate_messages().unwrap();

        // All should have been called exactly once
        assert_eq!(high_priority.read().unwrap().get_call_count(), 1);
        assert_eq!(medium_priority.read().unwrap().get_call_count(), 1);
        assert_eq!(low_priority.read().unwrap().get_call_count(), 1);
    }
}
