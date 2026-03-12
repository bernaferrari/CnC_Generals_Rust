//! Command Execution Bridge
//!
//! This module implements the bridge between the network layer and game logic.
//! It handles command execution, validation, and error reporting.

#[allow(unused_imports)]
use super::game_state::{GameState, GameStateResult, PlayerId};
use crate::commands::GameCommandData;
use crate::error::{NetworkError, NetworkResult};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, warn};

/// Statistics about command execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub validation_failures: u64,
}

/// Command executor that bridges network layer with game logic
///
/// The CommandExecutor is responsible for:
/// - Validating commands before execution
/// - Executing commands on the game state
/// - Tracking execution statistics
/// - Handling execution errors
pub struct CommandExecutor<G: GameState> {
    game_state: Arc<Mutex<G>>,
    stats: ExecutionStats,
    enable_validation: bool,
}

impl<G: GameState> CommandExecutor<G> {
    /// Create a new command executor
    pub fn new(game_state: Arc<Mutex<G>>) -> Self {
        Self {
            game_state,
            stats: ExecutionStats::default(),
            enable_validation: true,
        }
    }

    /// Create a new command executor with validation disabled
    ///
    /// This can be used for testing or when validation is performed elsewhere.
    pub fn new_without_validation(game_state: Arc<Mutex<G>>) -> Self {
        Self {
            game_state,
            stats: ExecutionStats::default(),
            enable_validation: false,
        }
    }

    /// Execute a single network command
    ///
    /// # Arguments
    /// * `command` - The command data from the network
    /// * `player_id` - The player who issued the command
    ///
    /// # Returns
    /// * `Ok(())` if command executed successfully
    /// * `Err(NetworkError)` if execution failed
    pub fn execute_command(
        &mut self,
        command: &GameCommandData,
        player_id: PlayerId,
    ) -> NetworkResult<()> {
        self.stats.total_commands += 1;

        debug!(
            "Executing command type {} from player {} for entity {:?}",
            command.command_type, player_id, command.target_id
        );

        // Lock game state
        let mut game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        // Validate command if enabled
        if self.enable_validation {
            match game.validate_command(command) {
                Ok(_) => {}
                Err(e) => {
                    self.stats.validation_failures += 1;
                    warn!("Command validation failed for player {}: {}", player_id, e);
                    return Err(e.into());
                }
            }
        }

        // Execute command
        match game.execute_command(command) {
            Ok(_) => {
                self.stats.successful_commands += 1;
                debug!(
                    "Successfully executed command type {} for player {}",
                    command.command_type, player_id
                );
                Ok(())
            }
            Err(e) => {
                self.stats.failed_commands += 1;
                error!("Command execution failed for player {}: {}", player_id, e);
                Err(e.into())
            }
        }
    }

    /// Execute multiple commands in order
    ///
    /// Commands are executed in the order provided. If any command fails,
    /// execution stops and an error is returned.
    ///
    /// # Arguments
    /// * `commands` - List of (player_id, command) tuples
    ///
    /// # Returns
    /// * `Ok(count)` - Number of commands executed successfully
    /// * `Err(NetworkError)` - First error encountered
    pub fn execute_commands(
        &mut self,
        commands: &[(PlayerId, GameCommandData)],
    ) -> NetworkResult<usize> {
        let mut executed = 0;

        for (player_id, command) in commands {
            self.execute_command(command, *player_id)?;
            executed += 1;
        }

        Ok(executed)
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> &ExecutionStats {
        &self.stats
    }

    /// Reset execution statistics
    pub fn reset_stats(&mut self) {
        self.stats = ExecutionStats::default();
    }

    /// Enable or disable command validation
    pub fn set_validation_enabled(&mut self, enabled: bool) {
        self.enable_validation = enabled;
    }

    /// Check if validation is enabled
    pub fn is_validation_enabled(&self) -> bool {
        self.enable_validation
    }

    /// Get reference to game state (for reading)
    ///
    /// This locks the game state, so the lock should be released quickly.
    pub fn with_game_state<F, R>(&self, f: F) -> NetworkResult<R>
    where
        F: FnOnce(&G) -> R,
    {
        let game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        Ok(f(&*game))
    }

    /// Get mutable reference to game state (for writing)
    ///
    /// This locks the game state, so the lock should be released quickly.
    pub fn with_game_state_mut<F, R>(&mut self, f: F) -> NetworkResult<R>
    where
        F: FnOnce(&mut G) -> R,
    {
        let mut game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        Ok(f(&mut *game))
    }
}

/// Batch command executor for processing multiple frames at once
///
/// This is useful for catching up after lag or processing recorded replays.
pub struct BatchCommandExecutor<G: GameState> {
    executor: CommandExecutor<G>,
    commands_per_frame: Vec<(u32, Vec<(PlayerId, GameCommandData)>)>,
}

impl<G: GameState> BatchCommandExecutor<G> {
    /// Create a new batch executor
    pub fn new(game_state: Arc<Mutex<G>>) -> Self {
        Self {
            executor: CommandExecutor::new(game_state),
            commands_per_frame: Vec::new(),
        }
    }

    /// Add commands for a frame
    pub fn add_frame(&mut self, frame: u32, commands: Vec<(PlayerId, GameCommandData)>) {
        self.commands_per_frame.push((frame, commands));
    }

    /// Execute all frames in order
    ///
    /// Frames are sorted by frame number before execution.
    pub fn execute_all(&mut self) -> NetworkResult<usize> {
        // Sort by frame number
        self.commands_per_frame.sort_by_key(|(frame, _)| *frame);

        let mut total_executed = 0;

        for (frame, commands) in &self.commands_per_frame {
            debug!("Executing batch frame {}", frame);

            match self.executor.execute_commands(commands) {
                Ok(count) => {
                    total_executed += count;
                    // Advance frame after all commands executed
                    self.executor
                        .with_game_state_mut(|game| game.advance_frame())?;
                }
                Err(e) => {
                    error!("Batch execution failed at frame {}: {}", frame, e);
                    return Err(e);
                }
            }
        }

        Ok(total_executed)
    }

    /// Clear all pending frames
    pub fn clear(&mut self) {
        self.commands_per_frame.clear();
    }

    /// Get number of pending frames
    pub fn pending_frames(&self) -> usize {
        self.commands_per_frame.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::game_state::{EntitySnapshot, GameStateCRC, ResourceState};
    use std::collections::BTreeMap;

    // Mock game state for testing
    struct MockGameState {
        frame: u32,
        entities: Vec<EntitySnapshot>,
        resources: BTreeMap<PlayerId, ResourceState>,
        executed_commands: Vec<GameCommandData>,
        should_fail: bool,
    }

    impl MockGameState {
        fn new() -> Self {
            Self {
                frame: 0,
                entities: Vec::new(),
                resources: BTreeMap::new(),
                executed_commands: Vec::new(),
                should_fail: false,
            }
        }
    }

    impl GameState for MockGameState {
        fn get_state_for_crc(&self) -> GameStateCRC {
            GameStateCRC {
                frame: self.frame,
                entities: self.entities.clone(),
                resources: self.resources.clone(),
                random_seed: 12345,
            }
        }

        fn execute_command(&mut self, command: &GameCommandData) -> GameStateResult<()> {
            if self.should_fail {
                return Err(super::super::game_state::GameStateError::ExecutionFailed(
                    "Mock failure".to_string(),
                ));
            }

            self.executed_commands.push(command.clone());
            Ok(())
        }

        fn current_frame(&self) -> u32 {
            self.frame
        }

        fn advance_frame(&mut self) {
            self.frame += 1;
        }

        fn get_entities(&self) -> Vec<EntitySnapshot> {
            self.entities.clone()
        }

        fn get_resources(&self) -> BTreeMap<PlayerId, ResourceState> {
            self.resources.clone()
        }

        fn get_random_seed(&self) -> u32 {
            12345
        }

        fn set_random_seed(&mut self, _seed: u32) {}

        fn entity_exists(&self, entity_id: u32) -> bool {
            self.entities.iter().any(|e| e.id == entity_id)
        }

        fn get_entity_owner(&self, entity_id: u32) -> Option<PlayerId> {
            self.entities
                .iter()
                .find(|e| e.id == entity_id)
                .map(|e| e.owner)
        }

        fn handle_desync(&mut self, _frame: u32, _local_crc: u32, _remote_crc: u32) {}
    }

    #[test]
    fn test_command_executor_creation() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let executor = CommandExecutor::new(game_state);

        assert!(executor.is_validation_enabled());
        assert_eq!(executor.get_stats().total_commands, 0);
    }

    #[test]
    fn test_execute_single_command() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut executor = CommandExecutor::new(game_state.clone());

        let command = GameCommandData {
            command_type: 1,
            target_id: Some(42),
            position: None,
            parameters: Default::default(),
            checksum: 0,
        };

        let result = executor.execute_command(&command, 0);
        assert!(result.is_ok());

        let stats = executor.get_stats();
        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.successful_commands, 1);
        assert_eq!(stats.failed_commands, 0);

        // Verify command was executed
        let game = game_state.lock().unwrap();
        assert_eq!(game.executed_commands.len(), 1);
    }

    #[test]
    fn test_execute_multiple_commands() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut executor = CommandExecutor::new(game_state.clone());

        let commands = vec![
            (
                0,
                GameCommandData {
                    command_type: 1,
                    target_id: Some(1),
                    position: None,
                    parameters: Default::default(),
                    checksum: 0,
                },
            ),
            (
                1,
                GameCommandData {
                    command_type: 2,
                    target_id: Some(2),
                    position: None,
                    parameters: Default::default(),
                    checksum: 0,
                },
            ),
        ];

        let result = executor.execute_commands(&commands);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        let game = game_state.lock().unwrap();
        assert_eq!(game.executed_commands.len(), 2);
    }

    #[test]
    fn test_execution_failure() {
        let mut mock = MockGameState::new();
        mock.should_fail = true;

        let game_state = Arc::new(Mutex::new(mock));
        let mut executor = CommandExecutor::new(game_state);

        let command = GameCommandData {
            command_type: 1,
            target_id: Some(42),
            position: None,
            parameters: Default::default(),
            checksum: 0,
        };

        let result = executor.execute_command(&command, 0);
        assert!(result.is_err());

        let stats = executor.get_stats();
        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.successful_commands, 0);
        assert_eq!(stats.failed_commands, 1);
    }

    #[test]
    fn test_batch_executor() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let mut batch_executor = BatchCommandExecutor::new(game_state.clone());

        // Add commands for frame 0
        batch_executor.add_frame(
            0,
            vec![(
                0,
                GameCommandData {
                    command_type: 1,
                    target_id: Some(1),
                    position: None,
                    parameters: Default::default(),
                    checksum: 0,
                },
            )],
        );

        // Add commands for frame 1
        batch_executor.add_frame(
            1,
            vec![(
                0,
                GameCommandData {
                    command_type: 2,
                    target_id: Some(2),
                    position: None,
                    parameters: Default::default(),
                    checksum: 0,
                },
            )],
        );

        let result = batch_executor.execute_all();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Verify frames advanced
        let game = game_state.lock().unwrap();
        assert_eq!(game.current_frame(), 2);
    }
}
