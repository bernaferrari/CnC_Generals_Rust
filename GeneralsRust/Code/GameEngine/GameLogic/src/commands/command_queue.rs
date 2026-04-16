////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Command Queue System - Command queuing and prioritization
//!
//! This module provides the command queuing system that manages command
//! execution order, priorities, and network synchronization.
//! Matches C++ CommandList and GameMessageList functionality.

use std::any::Any;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::command::{Command, CommandType, CommandValidation};
use super::rts_command::RtsCommand;
use crate::common::{AsciiString, Bool, Int, UnsignedInt};

/// Maximum commands that can be queued per player - matches C++ limits
pub const MAX_COMMANDS_PER_PLAYER: usize = 1000;

/// Maximum commands that can be executed per frame - prevents lag spikes
pub const MAX_COMMANDS_PER_FRAME: usize = 50;

/// Maximum size of the command queue - matches C++ MAX_COMMAND_QUEUE_SIZE
pub const MAX_COMMAND_QUEUE_SIZE: usize = 10000;

/// Command execution states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandExecutionState {
    Queued,
    Executing,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

/// Command priority levels - matches C++ command system
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPriority {
    Critical = 1000, // System commands that must execute immediately
    High = 800,      // Important commands like stop, emergency actions
    Normal = 500,    // Regular player commands
    Low = 200,       // Background tasks, cleanup
    Deferred = 0,    // Non-essential commands
}

impl Default for CommandPriority {
    fn default() -> Self {
        CommandPriority::Normal
    }
}

/// Queued command wrapper with metadata
#[derive(Debug, Clone)]
pub struct QueuedCommand {
    /// The actual command
    pub command: Command,

    /// Optional RTS-specific data
    pub rts_data: Option<RtsCommand>,

    /// Command priority for execution ordering
    pub priority: CommandPriority,

    /// Current execution state
    pub state: CommandExecutionState,

    /// Frame when command was queued
    pub queued_frame: UnsignedInt,

    /// Frame when command should execute (for delayed commands)
    pub execute_frame: UnsignedInt,

    /// Number of execution attempts
    pub retry_count: u8,

    /// Maximum retries allowed
    pub max_retries: u8,

    /// Execution timeout in frames
    pub timeout_frames: UnsignedInt,

    /// Last error message if execution failed
    pub error_message: Option<AsciiString>,

    /// User data for callbacks
    pub user_data: Option<Arc<dyn Any + Send + Sync>>,
}

impl QueuedCommand {
    /// Create new queued command
    pub fn new(command: Command, priority: CommandPriority, current_frame: UnsignedInt) -> Self {
        Self {
            command,
            rts_data: None,
            priority,
            state: CommandExecutionState::Queued,
            queued_frame: current_frame,
            execute_frame: current_frame, // Execute immediately by default
            retry_count: 0,
            max_retries: 3,
            timeout_frames: 300, // 10 seconds at 30 FPS
            error_message: None,
            user_data: None,
        }
    }

    /// Create from RTS command
    pub fn from_rts_command(rts_command: RtsCommand, current_frame: UnsignedInt) -> Self {
        let priority = Self::determine_priority(&rts_command);
        let mut queued = Self::new(rts_command.base_command.clone(), priority, current_frame);
        queued.rts_data = Some(rts_command);
        queued
    }

    /// Determine priority based on command type
    fn determine_priority(rts_command: &RtsCommand) -> CommandPriority {
        match rts_command.get_command_type() {
            // Critical system commands
            CommandType::DoStop | CommandType::DoScatter => CommandPriority::Critical,

            // High priority combat and emergency commands
            CommandType::DoAttackObject
            | CommandType::DoForceAttackObject
            | CommandType::DoForceAttackGround
            | CommandType::SelfDestruct => CommandPriority::High,

            // Normal gameplay commands
            CommandType::DoMoveTo
            | CommandType::DoAttackMoveTo
            | CommandType::QueueUnitCreate
            | CommandType::DozerConstruct => CommandPriority::Normal,

            // Low priority interface commands
            CommandType::CreateSelectedGroup | CommandType::AreaSelection => CommandPriority::Low,

            // Deferred commands
            CommandType::PlaceBeacon | CommandType::RemoveBeacon => CommandPriority::Deferred,

            _ => CommandPriority::Normal,
        }
    }

    /// Check if command can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries && self.state == CommandExecutionState::Failed
    }

    /// Mark for retry
    pub fn mark_for_retry(&mut self) {
        if self.can_retry() {
            self.retry_count += 1;
            self.state = CommandExecutionState::Queued;
        }
    }

    /// Check if command has timed out
    pub fn is_timed_out(&self, current_frame: UnsignedInt) -> bool {
        if self.state != CommandExecutionState::Executing {
            return false;
        }

        current_frame > self.execute_frame + self.timeout_frames
    }

    /// Get command ID
    pub fn get_id(&self) -> UnsignedInt {
        self.command.get_id()
    }

    /// Set delay before execution
    pub fn set_delayed_execution(&mut self, delay_frames: UnsignedInt) {
        self.execute_frame = self.queued_frame + delay_frames;
    }
}

// Priority queue ordering - higher priority commands come first
impl PartialEq for QueuedCommand {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.execute_frame == other.execute_frame
    }
}

impl Eq for QueuedCommand {}

impl PartialOrd for QueuedCommand {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedCommand {
    fn cmp(&self, other: &Self) -> Ordering {
        // `BinaryHeap` pops the greatest element first.
        // Higher priority should therefore compare as "greater", and earlier execute frames
        // should also compare as "greater" when priorities are equal.
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.execute_frame.cmp(&self.execute_frame))
            .then_with(|| other.queued_frame.cmp(&self.queued_frame))
    }
}

/// Command queue for a single player - matches C++ CommandList behavior
#[derive(Debug)]
pub struct PlayerCommandQueue {
    /// Priority queue of commands waiting to execute
    pending_commands: BinaryHeap<QueuedCommand>,

    /// Commands currently being executed
    executing_commands: HashMap<UnsignedInt, QueuedCommand>,

    /// Recently completed commands (for debugging/undo)
    completed_commands: VecDeque<QueuedCommand>,

    /// Player ID this queue belongs to
    player_id: Int,

    /// Current game frame
    current_frame: UnsignedInt,

    /// Statistics
    total_queued: u64,
    total_executed: u64,
    total_failed: u64,

    /// Queue settings
    max_completed_history: usize,
    enabled: bool,
}

impl PlayerCommandQueue {
    /// Create new player command queue
    pub fn new(player_id: Int) -> Self {
        Self {
            pending_commands: BinaryHeap::new(),
            executing_commands: HashMap::new(),
            completed_commands: VecDeque::new(),
            player_id,
            current_frame: 0,
            total_queued: 0,
            total_executed: 0,
            total_failed: 0,
            max_completed_history: 100,
            enabled: true,
        }
    }

    /// Queue a command for execution
    pub fn queue_command(&mut self, mut queued_command: QueuedCommand) -> Result<(), AsciiString> {
        if !self.enabled {
            return Err(AsciiString::from("Command queue is disabled"));
        }

        if self.pending_commands.len() >= MAX_COMMANDS_PER_PLAYER {
            return Err(AsciiString::from("Command queue is full"));
        }

        // Set current frame for timing
        queued_command.queued_frame = self.current_frame;
        if queued_command.execute_frame == 0 {
            queued_command.execute_frame = self.current_frame;
        }

        self.pending_commands.push(queued_command);
        self.total_queued += 1;

        Ok(())
    }

    /// Update queue for current frame - returns commands ready to execute
    pub fn update(&mut self, current_frame: UnsignedInt) -> Vec<QueuedCommand> {
        self.current_frame = current_frame;

        // Check for timed out executing commands
        self.check_timeouts();

        // Get commands ready to execute
        let mut ready_commands = Vec::new();
        let mut commands_this_frame = 0;

        while let Some(command) = self.pending_commands.peek() {
            // Check if command is ready to execute
            if command.execute_frame <= current_frame
                && commands_this_frame < MAX_COMMANDS_PER_FRAME
            {
                if let Some(mut command) = self.pending_commands.pop() {
                    command.state = CommandExecutionState::Executing;

                    let command_id = command.get_id();
                    self.executing_commands.insert(command_id, command.clone());
                    ready_commands.push(command);

                    commands_this_frame += 1;
                } else {
                    break;
                }
            } else {
                break; // No more commands ready this frame
            }
        }

        ready_commands
    }

    /// Mark command as completed
    pub fn complete_command(
        &mut self,
        command_id: UnsignedInt,
        success: bool,
        error: Option<AsciiString>,
    ) {
        if let Some(mut command) = self.executing_commands.remove(&command_id) {
            command.state = if success {
                CommandExecutionState::Completed
            } else {
                CommandExecutionState::Failed
            };

            if let Some(error_msg) = error {
                command.error_message = Some(error_msg);
            }

            // Update statistics
            if success {
                self.total_executed += 1;
            } else {
                self.total_failed += 1;

                // Try to retry failed command
                if command.can_retry() {
                    command.mark_for_retry();
                    self.pending_commands.push(command);
                    return;
                }
            }

            // Add to completed history
            self.completed_commands.push_back(command);

            // Trim history if too long
            while self.completed_commands.len() > self.max_completed_history {
                self.completed_commands.pop_front();
            }
        }
    }

    /// Cancel command by ID
    pub fn cancel_command(&mut self, command_id: UnsignedInt) -> bool {
        // Check executing commands
        if let Some(mut command) = self.executing_commands.remove(&command_id) {
            command.state = CommandExecutionState::Cancelled;
            self.completed_commands.push_back(command);
            return true;
        }

        // Check pending commands - this is inefficient but needed for cancellation
        let mut temp_commands = Vec::new();
        let mut found = false;

        while let Some(command) = self.pending_commands.pop() {
            if command.get_id() == command_id && !found {
                let mut cancelled_command = command;
                cancelled_command.state = CommandExecutionState::Cancelled;
                self.completed_commands.push_back(cancelled_command);
                found = true;
            } else {
                temp_commands.push(command);
            }
        }

        // Put back remaining commands
        for command in temp_commands {
            self.pending_commands.push(command);
        }

        found
    }

    /// Cancel all commands of specific type
    pub fn cancel_commands_of_type(&mut self, command_type: CommandType) -> u32 {
        let mut cancelled_count = 0;

        // Cancel executing commands
        let executing_ids: Vec<UnsignedInt> = self
            .executing_commands
            .iter()
            .filter(|(_, cmd)| cmd.command.get_type() == command_type)
            .map(|(id, _)| *id)
            .collect();

        for command_id in executing_ids {
            if self.cancel_command(command_id) {
                cancelled_count += 1;
            }
        }

        // Cancel pending commands
        let mut temp_commands = Vec::new();

        while let Some(command) = self.pending_commands.pop() {
            if command.command.get_type() == command_type {
                let mut cancelled_command = command;
                cancelled_command.state = CommandExecutionState::Cancelled;
                self.completed_commands.push_back(cancelled_command);
                cancelled_count += 1;
            } else {
                temp_commands.push(command);
            }
        }

        // Put back remaining commands
        for command in temp_commands {
            self.pending_commands.push(command);
        }

        cancelled_count
    }

    /// Clear all commands
    pub fn clear_all(&mut self) {
        self.pending_commands.clear();
        self.executing_commands.clear();
    }

    /// Get queue statistics
    pub fn get_stats(&self) -> PlayerCommandQueueStats {
        PlayerCommandQueueStats {
            pending_count: self.pending_commands.len(),
            executing_count: self.executing_commands.len(),
            completed_count: self.completed_commands.len(),
            total_queued: self.total_queued,
            total_executed: self.total_executed,
            total_failed: self.total_failed,
        }
    }

    /// Enable/disable queue processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check for and handle timed out commands
    fn check_timeouts(&mut self) {
        let timed_out_ids: Vec<UnsignedInt> = self
            .executing_commands
            .iter()
            .filter(|(_, cmd)| cmd.is_timed_out(self.current_frame))
            .map(|(id, _)| *id)
            .collect();

        for command_id in timed_out_ids {
            if let Some(mut command) = self.executing_commands.remove(&command_id) {
                command.state = CommandExecutionState::TimedOut;
                command.error_message = Some(AsciiString::from("Command execution timed out"));
                self.completed_commands.push_back(command);
                self.total_failed += 1;
            }
        }
    }
}

/// Command queue statistics
#[derive(Debug, Clone)]
pub struct PlayerCommandQueueStats {
    pub pending_count: usize,
    pub executing_count: usize,
    pub completed_count: usize,
    pub total_queued: u64,
    pub total_executed: u64,
    pub total_failed: u64,
}

/// Global command queue manager - manages all player queues
#[derive(Debug)]
pub struct CommandQueueManager {
    /// Command queues for each player
    player_queues: HashMap<Int, PlayerCommandQueue>,

    /// System command queue (no player association)
    system_queue: PlayerCommandQueue,

    /// Current game frame
    current_frame: UnsignedInt,

    /// Global settings
    enabled: bool,
    max_players: Int,
}

impl CommandQueueManager {
    /// Create new command queue manager
    pub fn new(max_players: Int) -> Self {
        Self {
            player_queues: HashMap::new(),
            system_queue: PlayerCommandQueue::new(-1), // -1 for system
            current_frame: 0,
            enabled: true,
            max_players,
        }
    }

    /// Initialize player queue
    pub fn initialize_player(&mut self, player_id: Int) -> Result<(), AsciiString> {
        if player_id < 0 || player_id >= self.max_players {
            return Err(AsciiString::from(&format!(
                "Invalid player ID: {}",
                player_id
            )));
        }

        if self.player_queues.contains_key(&player_id) {
            return Err(AsciiString::from(&format!(
                "Player {} already initialized",
                player_id
            )));
        }

        self.player_queues
            .insert(player_id, PlayerCommandQueue::new(player_id));
        Ok(())
    }

    /// Queue command for player
    pub fn queue_player_command(
        &mut self,
        player_id: Int,
        command: QueuedCommand,
    ) -> Result<(), AsciiString> {
        if !self.enabled {
            return Err(AsciiString::from("Command queue manager is disabled"));
        }

        let queue = self
            .player_queues
            .get_mut(&player_id)
            .ok_or_else(|| AsciiString::from(&format!("Player {} not initialized", player_id)))?;

        queue.queue_command(command)
    }

    /// Queue system command
    pub fn queue_system_command(&mut self, command: QueuedCommand) -> Result<(), AsciiString> {
        if !self.enabled {
            return Err(AsciiString::from("Command queue manager is disabled"));
        }

        self.system_queue.queue_command(command)
    }

    /// Update all queues for current frame
    pub fn update_frame(&mut self, current_frame: UnsignedInt) -> HashMap<Int, Vec<QueuedCommand>> {
        self.current_frame = current_frame;
        let mut ready_commands = HashMap::new();

        if !self.enabled {
            return ready_commands;
        }

        // Update system queue first
        let system_commands = self.system_queue.update(current_frame);
        if !system_commands.is_empty() {
            ready_commands.insert(-1, system_commands);
        }

        // Update player queues
        for (player_id, queue) in &mut self.player_queues {
            let player_commands = queue.update(current_frame);
            if !player_commands.is_empty() {
                ready_commands.insert(*player_id, player_commands);
            }
        }

        ready_commands
    }

    /// Complete command execution
    pub fn complete_command(
        &mut self,
        player_id: Int,
        command_id: UnsignedInt,
        success: bool,
        error: Option<AsciiString>,
    ) {
        if player_id == -1 {
            self.system_queue
                .complete_command(command_id, success, error);
        } else if let Some(queue) = self.player_queues.get_mut(&player_id) {
            queue.complete_command(command_id, success, error);
        }
    }

    /// Cancel command
    pub fn cancel_command(&mut self, player_id: Int, command_id: UnsignedInt) -> bool {
        if player_id == -1 {
            self.system_queue.cancel_command(command_id)
        } else if let Some(queue) = self.player_queues.get_mut(&player_id) {
            queue.cancel_command(command_id)
        } else {
            false
        }
    }

    /// Cancel all commands for player
    pub fn cancel_all_player_commands(&mut self, player_id: Int) {
        if let Some(queue) = self.player_queues.get_mut(&player_id) {
            queue.clear_all();
        }
    }

    /// Get queue statistics for player
    pub fn get_player_stats(&self, player_id: Int) -> Option<PlayerCommandQueueStats> {
        if player_id == -1 {
            Some(self.system_queue.get_stats())
        } else {
            self.player_queues
                .get(&player_id)
                .map(|queue| queue.get_stats())
        }
    }

    /// Get all player statistics
    pub fn get_all_stats(&self) -> HashMap<Int, PlayerCommandQueueStats> {
        let mut stats = HashMap::new();

        stats.insert(-1, self.system_queue.get_stats());

        for (player_id, queue) in &self.player_queues {
            stats.insert(*player_id, queue.get_stats());
        }

        stats
    }

    /// Update the maximum number of supported players. Existing queues with IDs
    /// greater than the new limit are removed.
    pub fn set_max_players(&mut self, max_players: Int) {
        self.max_players = max_players;
        self.player_queues
            .retain(|player_id, _| *player_id >= 0 && *player_id < max_players);
    }

    /// Clear all queued commands for every player and the system queue.
    pub fn clear_all(&mut self) {
        self.system_queue.clear_all();
        for queue in self.player_queues.values_mut() {
            queue.clear_all();
        }
    }

    /// Enable/disable queue processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        self.system_queue.set_enabled(enabled);
        for queue in self.player_queues.values_mut() {
            queue.set_enabled(enabled);
        }
    }

    /// Get current frame
    pub fn get_current_frame(&self) -> UnsignedInt {
        self.current_frame
    }
}

// Type alias for compatibility
pub type CommandQueue = PlayerCommandQueue;

/// Global command queue manager instance
use once_cell::sync::Lazy;
static COMMAND_QUEUE_MANAGER: Lazy<Arc<Mutex<CommandQueueManager>>> = Lazy::new(|| {
    Arc::new(Mutex::new(CommandQueueManager::new(8))) // Support up to 8 players
});

/// Get global command queue manager
pub fn get_command_queue_manager() -> Arc<Mutex<CommandQueueManager>> {
    COMMAND_QUEUE_MANAGER.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::command::Command;

    #[test]
    fn test_queued_command_priority() {
        let current_frame = 100;

        let high_priority = QueuedCommand::new(
            Command::new(CommandType::DoStop),
            CommandPriority::Critical,
            current_frame,
        );

        let low_priority = QueuedCommand::new(
            Command::new(CommandType::PlaceBeacon),
            CommandPriority::Low,
            current_frame,
        );

        // High priority should be "less than" low priority in ordering
        // (because BinaryHeap is a max-heap)
        assert!(high_priority > low_priority);
    }

    #[test]
    fn test_player_queue() {
        let mut queue = PlayerCommandQueue::new(1);

        let command = QueuedCommand::new(
            Command::new(CommandType::DoMoveTo),
            CommandPriority::Normal,
            0,
        );

        assert!(queue.queue_command(command).is_ok());
        assert_eq!(queue.get_stats().pending_count, 1);

        let ready_commands = queue.update(0);
        assert_eq!(ready_commands.len(), 1);
        assert_eq!(queue.get_stats().executing_count, 1);
    }

    #[test]
    fn test_command_cancellation() {
        let mut queue = PlayerCommandQueue::new(1);

        let command = QueuedCommand::new(
            Command::new(CommandType::DoMoveTo),
            CommandPriority::Normal,
            0,
        );
        let command_id = command.get_id();

        queue.queue_command(command).unwrap();
        assert!(queue.cancel_command(command_id));
        assert_eq!(queue.get_stats().pending_count, 0);
    }

    #[test]
    fn test_queue_manager() {
        let mut manager = CommandQueueManager::new(4);

        assert!(manager.initialize_player(0).is_ok());
        assert!(manager.initialize_player(0).is_err()); // Already initialized

        let command = QueuedCommand::new(
            Command::new(CommandType::DoAttackObject),
            CommandPriority::High,
            50,
        );

        assert!(manager.queue_player_command(0, command).is_ok());

        let ready_commands = manager.update_frame(50);
        assert!(ready_commands.contains_key(&0));
        assert_eq!(ready_commands[&0].len(), 1);
    }
}
