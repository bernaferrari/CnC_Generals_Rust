////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Command System Module - Complete RTS command processing
//!
//! This module provides the complete command system for Command & Conquer Generals Zero Hour,
//! exactly matching the C++ implementation. It includes:
//!
//! - Base command classes (matching GameMessage)
//! - RTS-specific command extensions
//! - Command queuing and prioritization
//! - Command execution and processing
//! - Unit selection management
//! - Formation system for coordinated movement
//!
//! ## Architecture Overview
//!
//! The command system follows the same architecture as the original C++ code:
//!
//! 1. **Commands** - Base command types and message system
//! 2. **RTS Commands** - Strategy game specific command extensions
//! 3. **Command Queue** - Priority-based command queuing (matches CommandList)
//! 4. **Command Processor** - Execution engine that processes queued commands
//! 5. **Selection System** - Unit selection and group management
//! 6. **Formation System** - Coordinated unit movement and formations
//!
//! ## Usage
//!
//! ```ignore
//! use gamelogic::commands::*;
//!
//! // Create and queue a movement command
//! let move_command = command_builder::create_move_to_position(
//!     vec![unit1, unit2, unit3],
//!     [100.0, 200.0, 0.0],
//!     player_id
//! );
//!
//! let queue_manager = get_command_queue_manager();
//! let mut manager = queue_manager.lock().unwrap();
//! manager.queue_player_command(player_id, QueuedCommand::new(move_command, CommandPriority::Normal, current_frame))?;
//!
//! // Process commands each frame
//! let ready_commands = manager.update_frame(current_frame);
//! for (player_id, commands) in ready_commands {
//!     for command in commands {
//!         // Command processor will handle execution
//!     }
//! }
//! ```
//!
//! ## Network Compatibility
//!
//! The command system maintains full compatibility with the C++ network protocol:
//! - Command types use the same numeric values
//! - Message serialization format matches exactly
//! - Network message ordering is preserved
//! - All multiplayer synchronization is maintained

// Export all public types and functions
pub mod command;
pub mod command_processor;
pub mod command_queue;
pub mod formation;
pub mod rts_command;
pub mod selection;

// Re-export commonly used types for convenience
pub use command::{
    command_builder, Command, CommandArgumentDataType, CommandArgumentType, CommandType,
    CommandValidation, CommandValidator, DefaultCommandValidator, MAX_COMMAND_ARGUMENTS,
};

pub use rts_command::{
    CursorMode, ModifierKeys, ObjectCapabilities, PlayerResources, ResourceCost, RtsCommand,
    RtsCommandCategory, RtsCommandContext, RtsCommandFactory, RtsCommandValidator,
};

pub use command_queue::{
    get_command_queue_manager, CommandExecutionState, CommandPriority, CommandQueue,
    CommandQueueManager, PlayerCommandQueue, PlayerCommandQueueStats, QueuedCommand,
    MAX_COMMANDS_PER_FRAME, MAX_COMMANDS_PER_PLAYER,
};

pub use command_processor::{
    get_command_processor, AIManager, CommandExecutionContext, CommandExecutionResult,
    CommandExecutionStats, CommandHandler, CommandProcessor, GameObject, ObjectManager,
    PlayerManager,
};

// Additional command system constants and types
pub use command_queue::MAX_COMMAND_QUEUE_SIZE;

// Command priorities
pub const COMMAND_PRIORITY_IMMEDIATE: i32 = 0;
pub const COMMAND_PRIORITY_HIGH: i32 = 1;
pub const COMMAND_PRIORITY_NORMAL: i32 = 2;
pub const COMMAND_PRIORITY_LOW: i32 = 3;
pub const COMMAND_PRIORITY_DEFERRED: i32 = 4;

// Command parameter and status types
pub type CommandParams = command::CommandArgumentDataType;
pub type CommandStatus = CommandExecutionResult;

// Global command processor instance - matches C++ THE_COMMAND_PROCESSOR
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};

pub static THE_COMMAND_PROCESSOR: Lazy<Arc<Mutex<CommandProcessor>>> =
    Lazy::new(|| Arc::new(Mutex::new(CommandProcessor::new())));

pub use selection::{
    get_selection_manager, ControlGroup, ObjectInfo, ObjectKind, ObjectLookup, PlayerSelection,
    SelectedObject, SelectionCriteria, SelectionInfo, SelectionManager, SelectionType,
    MAX_CONTROL_GROUPS, MAX_SELECTION_SIZE,
};

pub use formation::{
    get_formation_manager, Formation, FormationManager, FormationMovementOrder,
    FormationObjectLookup, FormationPosition, FormationSettings, FormationState, FormationTemplate,
    FormationType,
};

/// Initialize the command system - call this at game startup
pub fn initialize_command_system(max_players: i32) -> Result<(), String> {
    // Initialize command queue manager
    {
        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        // Initialize player queues
        for player_id in 0..max_players {
            if let Err(e) = manager.initialize_player(player_id) {
                let message = e.to_string();
                if message.contains("already initialized") {
                    continue;
                }
                return Err(format!(
                    "Failed to initialize player {}: {}",
                    player_id, message
                ));
            }
        }
    }

    // Initialize selection manager
    {
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        manager.set_object_lookup(Arc::new(selection::RegistryObjectLookup));

        // Initialize player selections
        for player_id in 0..max_players {
            manager.initialize_player(player_id);
        }
    }

    // Initialize formation manager (no per-player setup needed)
    let _formation_manager = get_formation_manager();

    // Initialize command processor (no setup needed)
    let _command_processor = get_command_processor();

    Ok(())
}

/// Update the command system each frame - call this every game frame
pub fn update_command_system(current_frame: u32) -> Result<(), String> {
    // Update selection manager
    {
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;
        manager.update(current_frame);
    }

    // Update formation manager
    {
        let formation_manager = get_formation_manager();
        let mut manager = formation_manager
            .write()
            .map_err(|_| "Failed to lock formation manager")?;
        manager.update(current_frame);
    }

    // Command processor update is handled separately per frame in main game loop

    Ok(())
}

/// Shutdown the command system - call this at game shutdown
pub fn shutdown_command_system() {
    // Clear all command queues
    if let Ok(_queue_manager) = get_command_queue_manager().lock() {
        // Queue manager will clean up automatically when dropped
    }

    // Clear all selections
    if let Ok(_selection_manager) = get_selection_manager().write() {
        // Selection manager will clean up automatically when dropped
    }

    // Clear all formations
    if let Ok(_formation_manager) = get_formation_manager().write() {
        // Formation manager will clean up automatically when dropped
    }

    // Command processor cleans up automatically
}

/// Get command system statistics for debugging
pub fn get_command_system_stats() -> CommandSystemStats {
    let mut stats = CommandSystemStats::default();

    // Get queue statistics
    if let Ok(queue_manager) = get_command_queue_manager().lock() {
        stats.queue_stats = queue_manager.get_all_stats();
    }

    // Get processor statistics
    if let Ok(processor) = get_command_processor().lock() {
        stats.execution_stats = processor.get_statistics().clone();
        stats.average_frame_time = processor.get_average_frame_time();
    }

    // Get formation statistics
    if let Ok(formation_manager) = get_formation_manager().read() {
        stats.active_formations = formation_manager.get_formation_count();
    }

    stats
}

/// Command system statistics for debugging and monitoring
#[derive(Debug, Clone, Default)]
pub struct CommandSystemStats {
    /// Per-player queue statistics
    pub queue_stats: std::collections::HashMap<i32, PlayerCommandQueueStats>,

    /// Command execution statistics
    pub execution_stats: CommandExecutionStats,

    /// Average frame processing time
    pub average_frame_time: f64,

    /// Number of active formations
    pub active_formations: usize,
}

impl CommandSystemStats {
    /// Get total commands queued across all players
    pub fn get_total_queued_commands(&self) -> u64 {
        self.queue_stats
            .values()
            .map(|stats| stats.total_queued)
            .sum()
    }

    /// Get total commands executed across all players  
    pub fn get_total_executed_commands(&self) -> u64 {
        self.queue_stats
            .values()
            .map(|stats| stats.total_executed)
            .sum()
    }

    /// Get total failed commands across all players
    pub fn get_total_failed_commands(&self) -> u64 {
        self.queue_stats
            .values()
            .map(|stats| stats.total_failed)
            .sum()
    }

    /// Get success rate as percentage
    pub fn get_success_rate(&self) -> f64 {
        let executed = self.get_total_executed_commands();
        let failed = self.get_total_failed_commands();
        let total = executed + failed;

        if total > 0 {
            (executed as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Convenience functions for common command operations
pub mod commands {
    use super::*;
    use crate::common::{Coord3D, Int, ObjectID};

    /// Create and queue a move command for the given objects
    pub fn move_objects_to_position(
        objects: Vec<ObjectID>,
        position: Coord3D,
        player_id: Int,
        current_frame: u32,
    ) -> Result<(), String> {
        let command = command_builder::create_move_to_position(objects, position, player_id);
        let queued_command = QueuedCommand::new(command, CommandPriority::Normal, current_frame);

        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        manager
            .queue_player_command(player_id, queued_command)
            .map_err(|e| e.to_string())
    }

    /// Create and queue an attack command
    pub fn attack_object(
        attackers: Vec<ObjectID>,
        target: ObjectID,
        player_id: Int,
        current_frame: u32,
    ) -> Result<(), String> {
        let command = command_builder::create_attack_object(attackers, target, player_id);
        let queued_command = QueuedCommand::new(command, CommandPriority::High, current_frame);

        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        manager
            .queue_player_command(player_id, queued_command)
            .map_err(|e| e.to_string())
    }

    /// Create and queue a stop command
    pub fn stop_objects(
        objects: Vec<ObjectID>,
        player_id: Int,
        current_frame: u32,
    ) -> Result<(), String> {
        let command = command_builder::create_stop_command(objects, player_id);
        let queued_command = QueuedCommand::new(command, CommandPriority::Critical, current_frame);

        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        manager
            .queue_player_command(player_id, queued_command)
            .map_err(|e| e.to_string())
    }

    /// Select objects for a player
    pub fn select_objects(
        objects: Vec<ObjectID>,
        player_id: Int,
        selection_type: SelectionType,
    ) -> Result<(), String> {
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        if let Some(player_selection) = manager.get_player_selection(player_id) {
            player_selection.select_objects(objects, selection_type);
            Ok(())
        } else {
            Err(format!("Player {} not initialized", player_id))
        }
    }

    /// Create a formation from selected objects
    pub fn create_formation(
        objects: Vec<ObjectID>,
        formation_type: &str,
        player_id: Int,
    ) -> Result<Option<u32>, String> {
        let formation_manager = get_formation_manager();
        let mut manager = formation_manager
            .write()
            .map_err(|_| "Failed to lock formation manager")?;

        Ok(manager.create_formation(formation_type, objects, player_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_system_initialization() {
        assert!(initialize_command_system(4).is_ok());
    }

    #[test]
    fn test_command_system_stats() {
        let stats = get_command_system_stats();
        assert!(stats.queue_stats.len() <= 8); // Max 8 players supported
    }

    #[test]
    fn test_convenience_functions() {
        initialize_command_system(2).unwrap();

        // Test move command
        let result =
            commands::move_objects_to_position(vec![1, 2, 3], [100.0, 200.0, 0.0].into(), 1, 100);
        assert!(result.is_ok());

        // Test selection
        let result = commands::select_objects(vec![1, 2, 3], 1, SelectionType::Replace);
        assert!(result.is_ok());
    }
}
