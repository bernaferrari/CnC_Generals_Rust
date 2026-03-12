//! Command List Implementation
//!
//! The CommandList is the final set of messages that have made their way through
//! all of the Translators of the MessageStream, and reached the end.
//! This set of commands will be executed by the GameLogic on its next iteration.

use super::game_message::*;
use super::message_stream::{GameMessageList, SubsystemInterface};
use log::{debug, error, info, warn};
use std::collections::LinkedList;
use std::sync::{Arc, RwLock};

/// The CommandList manages messages ready for execution by the game logic
pub struct CommandList {
    base: GameMessageList,
    max_commands_per_frame: usize,
    commands_processed_this_frame: usize,
}

impl CommandList {
    pub fn new() -> Self {
        Self {
            base: GameMessageList::new(),
            max_commands_per_frame: 1000, // Reasonable default limit
            commands_processed_this_frame: 0,
        }
    }

    /// Create a new CommandList with a specific command limit per frame
    pub fn with_limit(max_commands_per_frame: usize) -> Self {
        Self {
            base: GameMessageList::new(),
            max_commands_per_frame,
            commands_processed_this_frame: 0,
        }
    }

    /// Add a list of messages to the end of the command list
    pub fn append_message_list(&mut self, messages: Vec<GameMessage>) {
        debug!("Appending {} messages to command list", messages.len());

        for message in messages {
            self.base.append_message(message);
        }
    }

    /// Add a single message to the end of the command list
    pub fn append_message(&mut self, message: GameMessage) {
        debug!(
            "Appending single message to command list: {}",
            message.get_command_as_string()
        );
        self.base.append_message(message);
    }

    /// Get the next command to execute
    pub fn get_next_command(&mut self) -> Option<GameMessage> {
        if self.commands_processed_this_frame >= self.max_commands_per_frame {
            debug!(
                "Command limit reached for this frame ({}/{})",
                self.commands_processed_this_frame, self.max_commands_per_frame
            );
            return None;
        }

        if let Some(command) = self.pop_front_message() {
            self.commands_processed_this_frame += 1;
            debug!(
                "Retrieved command: {} ({}/{})",
                command.get_command_as_string(),
                self.commands_processed_this_frame,
                self.max_commands_per_frame
            );
            Some(command)
        } else {
            None
        }
    }

    /// Get all remaining commands up to the frame limit
    pub fn get_all_commands(&mut self) -> Vec<GameMessage> {
        let mut commands = Vec::new();

        while let Some(command) = self.get_next_command() {
            commands.push(command);
        }

        debug!("Retrieved {} commands for execution", commands.len());
        commands
    }

    /// Peek at the next command without removing it
    pub fn peek_next_command(&self) -> Option<&GameMessage> {
        if self.commands_processed_this_frame >= self.max_commands_per_frame {
            return None;
        }

        self.base.get_first_message()
    }

    /// Check if there are commands ready for execution
    pub fn has_commands(&self) -> bool {
        self.commands_processed_this_frame < self.max_commands_per_frame
            && self.base.message_count() > 0
    }

    /// Get the number of pending commands
    pub fn pending_command_count(&self) -> usize {
        self.base.message_count()
    }

    /// Get the maximum commands per frame limit
    pub fn get_max_commands_per_frame(&self) -> usize {
        self.max_commands_per_frame
    }

    /// Set the maximum commands per frame limit
    pub fn set_max_commands_per_frame(&mut self, limit: usize) {
        debug!("Setting command limit to {} per frame", limit);
        self.max_commands_per_frame = limit;
    }

    /// Get the number of commands processed this frame
    pub fn get_commands_processed_this_frame(&self) -> usize {
        self.commands_processed_this_frame
    }

    /// Reset the per-frame command counter (should be called each frame)
    pub fn reset_frame_counter(&mut self) {
        if self.commands_processed_this_frame > 0 {
            debug!(
                "Resetting frame counter. Processed {} commands last frame",
                self.commands_processed_this_frame
            );
        }
        self.commands_processed_this_frame = 0;
    }

    /// Check if there are specific types of commands in the list
    pub fn contains_command_of_type(&self, message_type: &GameMessageType) -> bool {
        self.base.contains_message_of_type(message_type)
    }

    /// Count commands of a specific type
    pub fn count_commands_of_type(&self, message_type: &GameMessageType) -> usize {
        self.base
            .iter()
            .filter(|msg| {
                std::mem::discriminant(msg.get_type()) == std::mem::discriminant(message_type)
            })
            .count()
    }

    /// Remove all commands of a specific type
    pub fn remove_commands_of_type(&mut self, message_type: &GameMessageType) -> usize {
        let initial_count = self.base.message_count();

        // Convert to Vec, filter, then rebuild the list
        let mut all_messages: Vec<_> = self.base.take_all_messages().into_iter().collect();
        all_messages.retain(|msg| {
            std::mem::discriminant(msg.get_type()) != std::mem::discriminant(message_type)
        });

        // Put the filtered messages back
        for msg in all_messages {
            self.base.append_message(msg);
        }

        let removed_count = initial_count - self.base.message_count();
        if removed_count > 0 {
            info!(
                "Removed {} commands of type {:?}",
                removed_count, message_type
            );
        }

        removed_count
    }

    /// Get statistics about the current command list
    pub fn get_statistics(&self) -> CommandListStatistics {
        let mut stats = CommandListStatistics::default();
        stats.total_commands = self.base.message_count();
        stats.commands_processed_this_frame = self.commands_processed_this_frame;
        stats.max_commands_per_frame = self.max_commands_per_frame;

        // Count different types of commands
        for message in self.base.iter() {
            match message.get_type() {
                GameMessageType::DoMoveTo(_)
                | GameMessageType::DoAttackMoveTo(_)
                | GameMessageType::DoForceMoveTO(_) => {
                    stats.movement_commands += 1;
                }
                GameMessageType::DoAttackObject(_)
                | GameMessageType::DoForceAttackObject(_)
                | GameMessageType::DoForceAttackGround(_) => {
                    stats.combat_commands += 1;
                }
                GameMessageType::DozerConstruct(_, _, _)
                | GameMessageType::DozerConstructLine(_, _, _, _)
                | GameMessageType::QueueUnitCreate(_) => {
                    stats.construction_commands += 1;
                }
                _ => {
                    stats.other_commands += 1;
                }
            }
        }

        stats
    }

    /// Snapshot all pending messages without consuming them.
    pub fn snapshot_messages(&self) -> Vec<GameMessage> {
        self.base.iter().cloned().collect()
    }

    /// Retain messages based on a predicate (in-place).
    pub fn retain_messages<F>(&mut self, mut keep: F)
    where
        F: FnMut(&GameMessage) -> bool,
    {
        let mut all_messages: Vec<_> = self.base.take_all_messages().into_iter().collect();
        all_messages.retain(|msg| keep(msg));
        for msg in all_messages {
            self.base.append_message(msg);
        }
    }

    /// Clear all pending commands (emergency use only)
    pub fn clear_all_commands(&mut self) {
        let count = self.base.message_count();
        if count > 0 {
            warn!("Clearing {} pending commands from command list", count);
            self.base.clear();
        }
    }

    /// Helper method to pop the first message from the list
    fn pop_front_message(&mut self) -> Option<GameMessage> {
        // Take all messages, pop the front, then put the rest back
        let mut all_messages = self.base.take_all_messages();
        let front_message = all_messages.pop_front();

        // Put the remaining messages back
        for msg in all_messages {
            self.base.append_message(msg);
        }

        front_message
    }

    /// Destroy all messages (implementation of the original C++ method)
    fn destroy_all_messages(&mut self) {
        debug!("Destroying all messages in command list");
        self.base.clear();
        self.commands_processed_this_frame = 0;
    }
}

impl Default for CommandList {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for CommandList {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing CommandList");
        self.base.init()
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Resetting CommandList");
        self.destroy_all_messages();
        self.base.reset()
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset the frame counter each update
        self.reset_frame_counter();
        Ok(())
    }
}

/// Statistics about the current state of the command list
#[derive(Debug, Default, Clone)]
pub struct CommandListStatistics {
    pub total_commands: usize,
    pub commands_processed_this_frame: usize,
    pub max_commands_per_frame: usize,
    pub movement_commands: usize,
    pub combat_commands: usize,
    pub construction_commands: usize,
    pub other_commands: usize,
}

impl CommandListStatistics {
    /// Check if the command list is at capacity for this frame
    pub fn is_at_capacity(&self) -> bool {
        self.commands_processed_this_frame >= self.max_commands_per_frame
    }

    /// Get the percentage of frame capacity used
    pub fn capacity_percentage(&self) -> f32 {
        if self.max_commands_per_frame == 0 {
            return 0.0;
        }
        (self.commands_processed_this_frame as f32 / self.max_commands_per_frame as f32) * 100.0
    }
}

/// Global command list instance
lazy_static::lazy_static! {
    pub static ref THE_COMMAND_LIST: Arc<RwLock<CommandList>> =
        Arc::new(RwLock::new(CommandList::new()));
}

/// Helper function to get the global command list
pub fn get_command_list() -> Arc<RwLock<CommandList>> {
    THE_COMMAND_LIST.clone()
}

/// Convenience function to append a message to the global command list
pub fn append_command(message: GameMessage) -> Result<(), Box<dyn std::error::Error>> {
    let command_list_arc = get_command_list();
    let mut command_list = command_list_arc
        .write()
        .map_err(|_| "Failed to acquire command list lock")?;
    command_list.append_message(message);
    Ok(())
}

/// Convenience function to get the next command from the global list
pub fn get_next_command() -> Result<Option<GameMessage>, Box<dyn std::error::Error>> {
    let command_list_arc = get_command_list();
    let mut command_list = command_list_arc
        .write()
        .map_err(|_| "Failed to acquire command list lock")?;
    Ok(command_list.get_next_command())
}

/// Convenience function to check if there are pending commands
pub fn has_pending_commands() -> Result<bool, Box<dyn std::error::Error>> {
    let command_list_arc = get_command_list();
    let command_list = command_list_arc
        .read()
        .map_err(|_| "Failed to acquire command list lock")?;
    Ok(command_list.has_commands())
}

/// Convenience function to get command list statistics
pub fn get_command_statistics() -> Result<CommandListStatistics, Box<dyn std::error::Error>> {
    let command_list_arc = get_command_list();
    let command_list = command_list_arc
        .read()
        .map_err(|_| "Failed to acquire command list lock")?;
    Ok(command_list.get_statistics())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_list_basic() {
        let mut command_list = CommandList::new();

        assert_eq!(command_list.pending_command_count(), 0);
        assert!(!command_list.has_commands());

        let msg = GameMessage::new(GameMessageType::Invalid);
        command_list.append_message(msg);

        assert_eq!(command_list.pending_command_count(), 1);
        assert!(command_list.has_commands());

        let retrieved = command_list.get_next_command();
        assert!(retrieved.is_some());
        assert_eq!(command_list.pending_command_count(), 0);
    }

    #[test]
    fn test_command_limit() {
        let mut command_list = CommandList::with_limit(2);

        // Add 3 commands
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));
        command_list.append_message(GameMessage::new(GameMessageType::NewGame));
        command_list.append_message(GameMessage::new(GameMessageType::ClearGameData));

        assert_eq!(command_list.pending_command_count(), 3);

        // Should only get 2 commands due to limit
        let commands = command_list.get_all_commands();
        assert_eq!(commands.len(), 2);
        assert_eq!(command_list.get_commands_processed_this_frame(), 2);
        assert_eq!(command_list.pending_command_count(), 1);

        // Try to get another command - should fail due to limit
        assert!(command_list.get_next_command().is_none());

        // Reset frame counter and try again
        command_list.reset_frame_counter();
        assert!(command_list.get_next_command().is_some());
    }

    #[test]
    fn test_message_list_operations() {
        let mut command_list = CommandList::new();

        let messages = vec![
            GameMessage::new(GameMessageType::Invalid),
            GameMessage::new(GameMessageType::NewGame),
            GameMessage::new(GameMessageType::ClearGameData),
        ];

        command_list.append_message_list(messages);
        assert_eq!(command_list.pending_command_count(), 3);

        // Test contains
        assert!(command_list.contains_command_of_type(&GameMessageType::Invalid));
        assert!(!command_list.contains_command_of_type(&GameMessageType::FrameTick(0)));

        // Test count
        assert_eq!(
            command_list.count_commands_of_type(&GameMessageType::Invalid),
            1
        );

        // Test remove
        let removed = command_list.remove_commands_of_type(&GameMessageType::Invalid);
        assert_eq!(removed, 1);
        assert_eq!(command_list.pending_command_count(), 2);
    }

    #[test]
    fn test_peek_functionality() {
        let mut command_list = CommandList::new();

        let msg = GameMessage::new(GameMessageType::Invalid);
        command_list.append_message(msg);

        // Peek should not remove the message
        let peeked = command_list.peek_next_command();
        assert!(peeked.is_some());
        assert_eq!(command_list.pending_command_count(), 1);

        // Get should remove the message
        let retrieved = command_list.get_next_command();
        assert!(retrieved.is_some());
        assert_eq!(command_list.pending_command_count(), 0);

        // Peek at empty list
        assert!(command_list.peek_next_command().is_none());
    }

    #[test]
    fn test_statistics() {
        let mut command_list = CommandList::with_limit(10);

        // Add different types of commands
        command_list.append_message(GameMessage::new(GameMessageType::DoMoveTo(
            Coord3D::default(),
        )));
        command_list.append_message(GameMessage::new(GameMessageType::DoAttackObject(456)));
        command_list.append_message(GameMessage::new(GameMessageType::DozerConstruct(
            789,
            Coord3D::default(),
            0.0,
        )));
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));

        let stats = command_list.get_statistics();
        assert_eq!(stats.total_commands, 4);
        assert_eq!(stats.movement_commands, 1);
        assert_eq!(stats.combat_commands, 1);
        assert_eq!(stats.construction_commands, 1);
        assert_eq!(stats.other_commands, 1);
        assert_eq!(stats.commands_processed_this_frame, 0);
        assert!(!stats.is_at_capacity());

        // Process some commands
        command_list.get_next_command();
        command_list.get_next_command();

        let stats2 = command_list.get_statistics();
        assert_eq!(stats2.commands_processed_this_frame, 2);
        assert_eq!(stats2.capacity_percentage(), 20.0); // 2/10 * 100
    }

    #[test]
    fn test_subsystem_interface() {
        let mut command_list = CommandList::new();

        assert!(command_list.init().is_ok());

        // Add some commands
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));
        command_list.get_next_command(); // Process one to increment counter

        assert_eq!(command_list.get_commands_processed_this_frame(), 1);

        // Update should reset frame counter
        assert!(command_list.update().is_ok());
        assert_eq!(command_list.get_commands_processed_this_frame(), 0);

        // Reset should clear everything
        assert!(command_list.reset().is_ok());
        assert_eq!(command_list.pending_command_count(), 0);
    }

    #[test]
    fn test_global_command_list() {
        // Test that we can get the global command list
        let list1 = get_command_list();
        let list2 = get_command_list();

        // Both should point to the same instance
        assert!(Arc::ptr_eq(&list1, &list2));

        // Test convenience functions
        let msg = GameMessage::new(GameMessageType::Invalid);
        assert!(append_command(msg).is_ok());
        assert!(has_pending_commands().unwrap());

        let retrieved = get_next_command().unwrap();
        assert!(retrieved.is_some());

        let stats = get_command_statistics().unwrap();
        assert_eq!(stats.commands_processed_this_frame, 1);
    }

    #[test]
    fn test_clear_all_commands() {
        let mut command_list = CommandList::new();

        // Add several commands
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));
        command_list.append_message(GameMessage::new(GameMessageType::NewGame));
        command_list.append_message(GameMessage::new(GameMessageType::ClearGameData));

        assert_eq!(command_list.pending_command_count(), 3);

        command_list.clear_all_commands();
        assert_eq!(command_list.pending_command_count(), 0);
        assert!(!command_list.has_commands());
    }

    #[test]
    fn test_frame_counter_edge_cases() {
        let mut command_list = CommandList::with_limit(1);

        // Add two commands
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));
        command_list.append_message(GameMessage::new(GameMessageType::NewGame));

        // Get first command - should work
        assert!(command_list.get_next_command().is_some());
        assert_eq!(command_list.get_commands_processed_this_frame(), 1);

        // Try to get second command - should fail due to limit
        assert!(command_list.get_next_command().is_none());

        // Peek should also fail when at limit
        assert!(command_list.peek_next_command().is_none());

        // But has_commands should return false even though there's a pending command
        assert!(!command_list.has_commands());

        // Reset and try again
        command_list.reset_frame_counter();
        assert!(command_list.has_commands());
        assert!(command_list.get_next_command().is_some());
    }
}
