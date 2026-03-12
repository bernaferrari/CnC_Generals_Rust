//! Game Loop Integration
//!
//! This module provides a complete game loop executor that integrates:
//! - Network frame synchronization
//! - Command execution
//! - CRC validation
//! - Desync detection
//!
//! It demonstrates the recommended pattern for integrating the network layer
//! with game logic in a deterministic lockstep simulation.

use super::command_executor::CommandExecutor;
use super::crc_validator::GameStateCRCValidator;
use super::desync_handler::{DesyncHandler, DesyncStatus, DesyncStrategy};
use super::game_state::{CRCValue, FrameNumber, GameState, PlayerId};
use crate::commands::GameCommandData;
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error};

/// Configuration for game loop
#[derive(Debug, Clone)]
pub struct GameLoopConfig {
    /// Target frames per second
    pub target_fps: u32,
    /// Interval for CRC validation (in frames)
    pub crc_interval: u32,
    /// Enable CRC validation
    pub enable_crc_validation: bool,
    /// Desync handling strategy
    pub desync_strategy: DesyncStrategy,
    /// Directory for state dumps (if any)
    pub dump_directory: Option<PathBuf>,
    /// Enable command validation before execution
    pub enable_command_validation: bool,
}

impl Default for GameLoopConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            crc_interval: 10, // Validate every 10 frames
            enable_crc_validation: true,
            desync_strategy: DesyncStrategy::DisconnectVote,
            dump_directory: None,
            enable_command_validation: true,
        }
    }
}

/// Statistics for game loop execution
#[derive(Debug, Clone, Default)]
pub struct GameLoopStats {
    pub frames_executed: u64,
    pub commands_executed: u64,
    pub crcs_computed: u64,
    pub desyncs_detected: u64,
    pub average_frame_time_ms: f64,
    pub max_frame_time_ms: f64,
}

/// Main game loop executor
///
/// This integrates all components needed for deterministic multiplayer:
/// - Frame synchronization
/// - Command execution
/// - CRC validation
/// - Desync detection
pub struct GameLoopExecutor<G: GameState> {
    game_state: Arc<Mutex<G>>,
    config: GameLoopConfig,
    command_executor: CommandExecutor<G>,
    crc_validator: GameStateCRCValidator<G>,
    desync_handler: DesyncHandler<G>,
    stats: GameLoopStats,
    frame_time_samples: Vec<f64>,
    max_frame_samples: usize,
}

impl<G: GameState> GameLoopExecutor<G> {
    /// Create a new game loop executor
    pub fn new(game_state: Arc<Mutex<G>>, config: GameLoopConfig) -> Self {
        let command_executor = CommandExecutor::new(game_state.clone());

        let crc_validator = GameStateCRCValidator::new(game_state.clone());

        let desync_handler = DesyncHandler::with_settings(
            game_state.clone(),
            config.desync_strategy,
            config.dump_directory.clone(),
        );

        Self {
            game_state,
            config,
            command_executor,
            crc_validator,
            desync_handler,
            stats: GameLoopStats::default(),
            frame_time_samples: Vec::new(),
            max_frame_samples: 100,
        }
    }

    /// Execute a single frame
    ///
    /// This method:
    /// 1. Executes all commands for the frame in deterministic order
    /// 2. Advances game state
    /// 3. Computes CRC if needed
    /// 4. Returns (executed, crc_option)
    pub fn execute_frame(
        &mut self,
        commands: Vec<(PlayerId, GameCommandData)>,
    ) -> NetworkResult<(FrameNumber, Option<CRCValue>)> {
        let start_time = NetworkInstant::now();

        // Get current frame before execution
        let frame = {
            let game = self
                .game_state
                .lock()
                .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;
            game.current_frame()
        };

        debug!("Executing frame {} with {} commands", frame, commands.len());

        // Execute all commands in order
        let executed_count = self.command_executor.execute_commands(&commands)?;

        self.stats.commands_executed += executed_count as u64;

        // Advance frame
        {
            let mut game = self
                .game_state
                .lock()
                .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;
            game.advance_frame();
        }

        self.stats.frames_executed += 1;

        // Compute CRC if needed
        let crc = if self.config.enable_crc_validation && frame % self.config.crc_interval == 0 {
            let crc_value = self.crc_validator.compute_crc(frame)?;
            self.stats.crcs_computed += 1;
            debug!("Frame {} CRC: {:08x}", frame, crc_value);
            Some(crc_value)
        } else {
            None
        };

        // Update timing stats
        let frame_time = start_time.elapsed().as_secs_f64() * 1000.0;
        self.update_frame_time(frame_time);

        Ok((frame, crc))
    }

    /// Validate CRCs from remote players
    ///
    /// Returns list of desynced players (if any)
    pub fn validate_crcs(
        &mut self,
        frame: FrameNumber,
        remote_crcs: HashMap<PlayerId, CRCValue>,
    ) -> NetworkResult<Vec<PlayerId>> {
        if !self.config.enable_crc_validation {
            return Ok(Vec::new());
        }

        // Compute local CRC
        let local_crc = self.crc_validator.compute_crc(frame)?;

        // Detect desync
        let status = self
            .desync_handler
            .detect_desync(frame, local_crc, remote_crcs.clone());

        match status {
            DesyncStatus::Synchronized => {
                debug!("Frame {} synchronized: CRC {:08x}", frame, local_crc);
                Ok(Vec::new())
            }
            DesyncStatus::Desynchronized {
                frame,
                desynced_players,
            } => {
                self.stats.desyncs_detected += 1;

                error!(
                    "Frame {} desynchronized! Players: {:?}",
                    frame, desynced_players
                );

                // Handle each desynced player
                for &player_id in &desynced_players {
                    if let Some(&remote_crc) = remote_crcs.get(&player_id) {
                        self.desync_handler
                            .handle_desync(frame, local_crc, remote_crc, player_id)?;
                    }
                }

                Ok(desynced_players)
            }
        }
    }

    /// Update frame timing statistics
    fn update_frame_time(&mut self, frame_time_ms: f64) {
        self.frame_time_samples.push(frame_time_ms);

        // Keep only recent samples
        if self.frame_time_samples.len() > self.max_frame_samples {
            self.frame_time_samples.remove(0);
        }

        // Update average
        let sum: f64 = self.frame_time_samples.iter().sum();
        self.stats.average_frame_time_ms = sum / self.frame_time_samples.len() as f64;

        // Update max
        if frame_time_ms > self.stats.max_frame_time_ms {
            self.stats.max_frame_time_ms = frame_time_ms;
        }
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &GameLoopStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = GameLoopStats::default();
        self.frame_time_samples.clear();
    }

    /// Get current frame number
    pub fn current_frame(&self) -> NetworkResult<FrameNumber> {
        let game = self
            .game_state
            .lock()
            .map_err(|e| NetworkError::generic(format!("Failed to lock game state: {}", e)))?;

        Ok(game.current_frame())
    }

    /// Get target frame duration
    pub fn target_frame_duration(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.config.target_fps as f64)
    }

    /// Check if frame time is within budget
    pub fn is_frame_time_acceptable(&self) -> bool {
        let target_ms = 1000.0 / self.config.target_fps as f64;
        self.stats.average_frame_time_ms < target_ms * 0.9 // Allow 90% of budget
    }

    /// Get desync history
    pub fn get_desync_history(&self) -> &[super::desync_handler::DesyncRecord] {
        self.desync_handler.get_desync_history()
    }
}

/// Example main loop pseudo-code
///
/// This demonstrates the recommended integration pattern.
pub mod example {

    /// Pseudo-code for main game loop
    ///
    /// ```rust,ignore
    /// // Initialize
    /// let game_state = Arc::new(Mutex::new(MyGameState::new()));
    /// let config = GameLoopConfig::default();
    /// let mut game_loop = GameLoopExecutor::new(game_state.clone(), config);
    /// let mut network = NetworkInterface::new();
    ///
    /// // Main loop
    /// loop {
    ///     let frame_start = NetworkInstant::now();
    ///
    ///     // 1. Check if frame is ready (all player commands received)
    ///     if !network.is_frame_ready(current_frame) {
    ///         // Wait for network
    ///         sleep(Duration::from_millis(1));
    ///         continue;
    ///     }
    ///
    ///     // 2. Get commands for this frame
    ///     let commands = network.get_frame_commands(current_frame);
    ///
    ///     // 3. Execute frame
    ///     let (frame, crc_opt) = game_loop.execute_frame(commands)?;
    ///
    ///     // 4. If CRC was computed, broadcast and validate
    ///     if let Some(local_crc) = crc_opt {
    ///         network.broadcast_crc(frame, local_crc)?;
    ///
    ///         // Wait for remote CRCs
    ///         if let Some(remote_crcs) = network.get_remote_crcs(frame) {
    ///             let desynced = game_loop.validate_crcs(frame, remote_crcs)?;
    ///
    ///             if !desynced.is_empty() {
    ///                 // Handle desync (disconnect, vote, etc.)
    ///                 for player_id in desynced {
    ///                     network.initiate_disconnect_vote(player_id)?;
    ///                 }
    ///             }
    ///         }
    ///     }
    ///
    ///     // 5. Render
    ///     renderer.render(&game_state)?;
    ///
    ///     // 6. Sleep to maintain target FPS
    ///     let elapsed = frame_start.elapsed();
    ///     let target = game_loop.target_frame_duration();
    ///     if elapsed < target {
    ///         sleep(target - elapsed);
    ///     }
    /// }
    /// ```
    pub struct ExampleLoop;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::game_state::{EntitySnapshot, GameStateCRC, ResourceState};
    use std::collections::BTreeMap;

    // Mock game state for testing
    struct MockGameState {
        frame: u32,
        executed_commands: Vec<GameCommandData>,
    }

    impl MockGameState {
        fn new() -> Self {
            Self {
                frame: 0,
                executed_commands: Vec::new(),
            }
        }
    }

    impl GameState for MockGameState {
        fn get_state_for_crc(&self) -> GameStateCRC {
            GameStateCRC {
                frame: self.frame,
                entities: Vec::new(),
                resources: BTreeMap::new(),
                random_seed: 12345,
            }
        }

        fn execute_command(
            &mut self,
            command: &GameCommandData,
        ) -> super::super::game_state::GameStateResult<()> {
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
            Vec::new()
        }

        fn get_resources(&self) -> BTreeMap<u8, ResourceState> {
            BTreeMap::new()
        }

        fn get_random_seed(&self) -> u32 {
            12345
        }

        fn set_random_seed(&mut self, _seed: u32) {}

        fn entity_exists(&self, _entity_id: u32) -> bool {
            false
        }

        fn get_entity_owner(&self, _entity_id: u32) -> Option<u8> {
            None
        }

        fn handle_desync(&mut self, _frame: u32, _local_crc: u32, _remote_crc: u32) {}
    }

    #[test]
    fn test_game_loop_executor_creation() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig::default();
        let executor = GameLoopExecutor::new(game_state, config);

        assert_eq!(executor.get_stats().frames_executed, 0);
    }

    #[test]
    fn test_execute_frame() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig::default();
        let mut executor = GameLoopExecutor::new(game_state.clone(), config);

        let commands = vec![(
            0,
            GameCommandData {
                command_type: 1,
                target_id: Some(42),
                position: None,
                parameters: Default::default(),
                checksum: 0,
            },
        )];

        let result = executor.execute_frame(commands);
        assert!(result.is_ok());

        let (frame, _crc) = result.unwrap();
        assert_eq!(frame, 0);

        assert_eq!(executor.get_stats().frames_executed, 1);
        assert_eq!(executor.get_stats().commands_executed, 1);

        // Verify frame advanced
        let game = game_state.lock().unwrap();
        assert_eq!(game.current_frame(), 1);
    }

    #[test]
    fn test_execute_multiple_frames() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig::default();
        let mut executor = GameLoopExecutor::new(game_state.clone(), config);

        for _ in 0..10 {
            let commands = vec![];
            executor.execute_frame(commands).unwrap();
        }

        assert_eq!(executor.get_stats().frames_executed, 10);

        let game = game_state.lock().unwrap();
        assert_eq!(game.current_frame(), 10);
    }

    #[test]
    fn test_crc_validation_synchronized() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig {
            crc_interval: 1, // Validate every frame
            ..Default::default()
        };
        let mut executor = GameLoopExecutor::new(game_state.clone(), config);

        // Execute frame
        let (frame, crc_opt) = executor.execute_frame(vec![]).unwrap();

        // Get local CRC
        let local_crc = crc_opt.unwrap();

        // Validate with matching CRC
        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(1, local_crc);

        let desynced = executor.validate_crcs(frame, remote_crcs).unwrap();

        assert!(desynced.is_empty());
    }

    #[test]
    fn test_crc_validation_desynchronized() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig {
            crc_interval: 1,
            ..Default::default()
        };
        let mut executor = GameLoopExecutor::new(game_state, config);

        // Execute frame
        let (frame, crc_opt) = executor.execute_frame(vec![]).unwrap();

        // Get local CRC
        let local_crc = crc_opt.unwrap();

        // Validate with different CRC
        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(1, local_crc.wrapping_add(1)); // Different CRC

        let desynced = executor.validate_crcs(frame, remote_crcs).unwrap();

        assert_eq!(desynced, vec![1]);
        assert_eq!(executor.get_stats().desyncs_detected, 1);
    }

    #[test]
    fn test_frame_timing_stats() {
        let game_state = Arc::new(Mutex::new(MockGameState::new()));
        let config = GameLoopConfig::default();
        let mut executor = GameLoopExecutor::new(game_state, config);

        // Execute some frames
        for _ in 0..5 {
            executor.execute_frame(vec![]).unwrap();
        }

        let stats = executor.get_stats();

        assert!(stats.average_frame_time_ms > 0.0);
        assert!(stats.max_frame_time_ms > 0.0);
        assert!(stats.max_frame_time_ms >= stats.average_frame_time_ms);
    }
}
