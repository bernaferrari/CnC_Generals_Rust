//! Frame-based deterministic networking system
//!
//! This module implements the deterministic networking system used by RTS games
//! to ensure all players stay synchronized. Commands are organized by execution
//! frame and processed in a deterministic order.
//!
//! # Synchronous vs Async Frame Execution
//!
//! The module provides two frame execution models:
//! - **Synchronous** (`sync_manager`): Matches C++ exactly, no async/await, deterministic
//! - **Async** (this file): Legacy async implementation for network I/O
//!
//! For deterministic RTS gameplay, use the synchronous model from `sync_manager`.

use crate::commands::{NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use crate::observability::telemetry;
use crate::time::NetworkInstant;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, trace, warn};

pub mod buffer;
pub mod crc;
pub mod frame_data;
pub mod frame_data_manager;
pub mod frame_metrics;
pub mod metrics;
pub mod sync_manager;
pub mod synchronization;
pub mod validation;

pub use buffer::{FrameBuffer, FrameReadyState};
pub use crc::{CRCValidator, FrameCRC, GameStateCRC, CRC};
pub use metrics::{FrameMetrics, MetricsSnapshot};
pub use sync_manager::{
    FrameData as SyncFrameData, FrameDataManager as SyncFrameDataManager, FrameDataReturnType,
    SyncFrameExecutor, FRAME_DATA_LENGTH, FRAME_TIME_MS, KEEPALIVE_INTERVAL_FRAMES,
    MAX_FRAMES_AHEAD, TARGET_FPS,
};

/// Frame data containing all commands for a specific game frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    /// Frame number
    pub frame_number: u32,
    /// All commands for this frame, organized by player
    pub player_commands: HashMap<u8, Vec<NetCommand>>,
    /// Frame timestamp
    pub timestamp: DateTime<Utc>,
    /// Frame checksum for validation
    pub checksum: u32,
    /// Total number of commands in frame
    pub total_commands: usize,
    /// Frame is complete (all expected players submitted)
    pub is_complete: bool,
    /// Frame has been executed
    pub is_executed: bool,
}

impl FrameData {
    /// Create new frame data
    pub fn new(frame_number: u32) -> Self {
        Self {
            frame_number,
            player_commands: HashMap::new(),
            timestamp: Utc::now(),
            checksum: 0,
            total_commands: 0,
            is_complete: false,
            is_executed: false,
        }
    }

    /// Add command to frame
    pub fn add_command(&mut self, command: NetCommand) -> NetworkResult<()> {
        // Validate command frame number matches
        if command.execution_frame != self.frame_number {
            return Err(NetworkError::frame_sync(format!(
                "command frame {} doesn't match frame data {}",
                command.execution_frame, self.frame_number
            )));
        }

        // Add to player's command list
        let player_commands = self
            .player_commands
            .entry(command.player_id)
            .or_insert_with(Vec::new);
        player_commands.push(command);

        // Update totals
        self.total_commands += 1;

        // Sort commands by sequence number for deterministic execution
        player_commands.sort_by_key(|cmd| cmd.sequence);

        Ok(())
    }

    /// Get all commands for a player in this frame
    pub fn get_player_commands(&self, player_id: u8) -> Vec<NetCommand> {
        self.player_commands
            .get(&player_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all commands in deterministic order
    pub fn get_all_commands_ordered(&self) -> Vec<NetCommand> {
        let mut all_commands = Vec::new();

        // Process players in order (0, 1, 2, ...)
        for player_id in 0..crate::config::MAX_PLAYERS {
            if let Some(commands) = self.player_commands.get(&player_id) {
                for command in commands {
                    all_commands.push(command.clone());
                }
            }
        }

        all_commands
    }

    /// Check if frame is ready for execution
    pub fn is_ready_for_execution(&self, expected_players: &[u8]) -> bool {
        // Frame is ready if we have commands from all expected active players
        // or if this is a timeout/forced execution

        if self.is_executed {
            return false;
        }

        // For now, simple check - would be more complex in real implementation
        expected_players.iter().all(|&player_id| {
            self.player_commands.contains_key(&player_id)
                || self.has_keepalive_from_player(player_id)
        })
    }

    /// Check if player sent at least a keepalive for this frame
    fn has_keepalive_from_player(&self, player_id: u8) -> bool {
        if let Some(commands) = self.player_commands.get(&player_id) {
            commands
                .iter()
                .any(|cmd| matches!(cmd.command_type, NetCommandType::KeepAlive))
        } else {
            false
        }
    }

    /// Mark frame as complete
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
        self.calculate_checksum();
    }

    /// Mark frame as executed
    pub fn mark_executed(&mut self) {
        self.is_executed = true;
    }

    /// Calculate deterministic checksum
    fn calculate_checksum(&mut self) {
        // Create deterministic representation for checksum
        let mut data = Vec::new();

        // Add frame number
        data.extend_from_slice(&self.frame_number.to_le_bytes());

        // Add commands in deterministic order
        for command in self.get_all_commands_ordered() {
            // Add key command data
            data.extend_from_slice(&(command.command_type as u8).to_le_bytes());
            data.extend_from_slice(&command.player_id.to_le_bytes());
            data.extend_from_slice(&command.sequence.to_le_bytes());

            // Add payload hash (simplified)
            let payload_size = command.payload.size();
            data.extend_from_slice(&payload_size.to_le_bytes());
        }

        // Calculate CRC32
        self.checksum = crc32fast::hash(&data);
    }

    /// Validate frame checksum
    pub fn validate_checksum(&self) -> bool {
        let mut temp_frame = self.clone();
        temp_frame.calculate_checksum();
        temp_frame.checksum == self.checksum
    }

    /// Get frame statistics
    pub fn get_stats(&self) -> FrameStats {
        let mut player_command_counts = HashMap::new();
        for (&player_id, commands) in &self.player_commands {
            player_command_counts.insert(player_id, commands.len());
        }

        FrameStats {
            frame_number: self.frame_number,
            total_commands: self.total_commands,
            player_command_counts,
            checksum: self.checksum,
            is_complete: self.is_complete,
            is_executed: self.is_executed,
            timestamp: self.timestamp,
        }
    }
}

/// Frame statistics
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    pub frame_number: u32,
    pub total_commands: usize,
    pub player_command_counts: HashMap<u8, usize>,
    pub checksum: u32,
    pub is_complete: bool,
    pub is_executed: bool,
    pub timestamp: DateTime<Utc>,
}

/// Frame data manager handles frame synchronization and execution
pub struct FrameDataManager {
    /// Configuration
    config: FrameManagerConfig,

    /// Current game frame
    current_frame: Arc<RwLock<u32>>,

    /// Frame data storage (frame_number -> FrameData)
    frames: Arc<RwLock<BTreeMap<u32, FrameData>>>,

    /// Expected players for frame synchronization
    expected_players: Arc<RwLock<Vec<u8>>>,

    /// Frame execution queue
    execution_queue: Arc<RwLock<VecDeque<u32>>>,

    /// Statistics
    stats: Arc<RwLock<FrameManagerStats>>,
    last_frame_exec: Arc<RwLock<Option<NetworkInstant>>>,

    /// Callback for frame execution
    frame_executor: Option<Arc<dyn FrameExecutor + Send + Sync>>,

    /// Execution task handle
    execution_task: Option<tokio::task::JoinHandle<()>>,
}

/// Frame manager configuration
#[derive(Debug, Clone)]
pub struct FrameManagerConfig {
    /// Maximum frames to keep in history
    pub max_frames_ahead: u32,
    /// Minimum run-ahead frames
    pub min_runahead: u32,
    /// Maximum frame history to keep
    pub max_frame_history: u32,
    /// Frame timeout in milliseconds
    pub frame_timeout_ms: u64,
    /// Enable frame validation
    pub enable_validation: bool,
    /// Target frames per second
    pub target_fps: u32,
}

impl Default for FrameManagerConfig {
    fn default() -> Self {
        Self {
            max_frames_ahead: 10,
            min_runahead: 2,
            max_frame_history: 100,
            frame_timeout_ms: 5000,
            enable_validation: true,
            target_fps: 30,
        }
    }
}

/// Frame manager statistics
#[derive(Debug, Clone, Default)]
pub struct FrameManagerStats {
    /// Total frames processed
    pub frames_processed: u64,
    /// Frames skipped due to timeout
    pub frames_skipped: u64,
    /// Frame sync errors
    pub sync_errors: u64,
    /// Average frame processing time
    pub avg_frame_time_ms: f64,
    /// Current frame rate
    pub current_fps: f64,
    /// Frames waiting for execution
    pub pending_frames: usize,
}

/// Trait for executing frames (object-safe version)
#[async_trait]
pub trait FrameExecutor: Send + Sync {
    /// Execute a frame with all its commands
    async fn execute_frame(&self, frame_data: &FrameData) -> NetworkResult<()>;

    /// Handle frame execution error
    async fn handle_frame_error(&self, frame_number: u32, error: NetworkError);
}

impl FrameDataManager {
    /// Create new frame data manager
    pub fn new(max_frames_ahead: u32, min_runahead: u32) -> Self {
        let config = FrameManagerConfig {
            max_frames_ahead,
            min_runahead,
            ..Default::default()
        };

        Self {
            config,
            current_frame: Arc::new(RwLock::new(0)),
            frames: Arc::new(RwLock::new(BTreeMap::new())),
            expected_players: Arc::new(RwLock::new(Vec::new())),
            execution_queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(FrameManagerStats::default())),
            last_frame_exec: Arc::new(RwLock::new(None)),
            frame_executor: None,
            execution_task: None,
        }
    }

    /// Reset all frame tracking state
    pub async fn reset(&mut self) -> NetworkResult<()> {
        info!("Resetting frame data manager");

        self.frames.write().await.clear();
        self.execution_queue.write().await.clear();
        *self.current_frame.write().await = 0;
        *self.stats.write().await = FrameManagerStats::default();

        if let Some(handle) = self.execution_task.take() {
            if !handle.is_finished() {
                handle.abort();
            }
        }

        Ok(())
    }

    /// Set frame executor
    pub fn set_frame_executor(&mut self, executor: Arc<dyn FrameExecutor + Send + Sync>) {
        self.frame_executor = Some(executor);
    }

    /// Update the target frames-per-second budget used for scheduling.
    pub fn set_target_fps(&mut self, target_fps: u32) {
        self.config.target_fps = target_fps.max(1);
    }

    /// Ensure a frame record exists for the provided number.
    pub async fn ensure_frame(&self, frame_number: u32) {
        let mut frames = self.frames.write().await;
        frames
            .entry(frame_number)
            .or_insert_with(|| FrameData::new(frame_number));
    }

    /// Ensure a contiguous window of frames exists in the buffer.
    pub async fn ensure_future_window(&self, start: u32, end: u32) {
        if start > end {
            return;
        }

        let mut frames = self.frames.write().await;
        for frame in start..=end {
            frames.entry(frame).or_insert_with(|| FrameData::new(frame));
        }
    }

    /// Set expected players
    pub async fn set_expected_players(&self, players: Vec<u8>) {
        let player_count = players.len();
        let mut expected = self.expected_players.write().await;
        *expected = players;
        info!("Set expected players: {:?}", *expected);
        if let Some(telemetry) = telemetry() {
            telemetry.set_active_players(player_count);
        }
    }

    /// Add command to appropriate frame
    pub async fn add_command(&self, command: NetCommand) -> NetworkResult<()> {
        let frame_number = command.execution_frame;

        // Validate frame number
        let current_frame = *self.current_frame.read().await;
        if frame_number < current_frame {
            return Err(NetworkError::frame_sync(format!(
                "command frame {} is in the past (current: {})",
                frame_number, current_frame
            )));
        }

        if frame_number > current_frame + self.config.max_frames_ahead {
            return Err(NetworkError::frame_sync(format!(
                "command frame {} is too far in future (max: {})",
                frame_number,
                current_frame + self.config.max_frames_ahead
            )));
        }

        // Get or create frame data
        {
            let mut frames = self.frames.write().await;
            let frame_data = frames
                .entry(frame_number)
                .or_insert_with(|| FrameData::new(frame_number));

            frame_data.add_command(command)?;

            // Check if frame is now ready for execution
            let expected_players = self.expected_players.read().await;
            if frame_data.is_ready_for_execution(&expected_players) && !frame_data.is_complete {
                frame_data.mark_complete();

                // Add to execution queue if it's the next frame to execute
                if frame_number == current_frame {
                    let mut queue = self.execution_queue.write().await;
                    queue.push_back(frame_number);
                }
            }
        }

        trace!("Added command to frame {}", frame_number);
        Ok(())
    }

    /// Return true when at least one frame is queued for execution.
    pub async fn has_ready_frame(&self) -> bool {
        let queue = self.execution_queue.read().await;
        !queue.is_empty()
    }

    /// Get frame data
    pub async fn get_frame(&self, frame_number: u32) -> Option<FrameData> {
        let frames = self.frames.read().await;
        frames.get(&frame_number).cloned()
    }

    /// Update frame manager (call regularly)
    pub async fn update(&self) -> NetworkResult<()> {
        // Execute ready frames
        self.execute_ready_frames().await?;

        // Clean up old frames
        self.cleanup_old_frames().await;

        // Update statistics
        self.update_statistics().await;

        Ok(())
    }

    /// Execute frames that are ready
    async fn execute_ready_frames(&self) -> NetworkResult<()> {
        let mut executed_count = 0;

        loop {
            let frame_to_execute = {
                let queue = self.execution_queue.read().await;
                queue.front().cloned()
            };

            if let Some(frame_number) = frame_to_execute {
                // Get frame data
                let frame_data = {
                    let frames = self.frames.read().await;
                    frames.get(&frame_number).cloned()
                };

                if let Some(mut frame_data) = frame_data {
                    if frame_data.is_complete && !frame_data.is_executed {
                        // Execute the frame
                        if let Some(executor) = &self.frame_executor {
                            let execute_start = NetworkInstant::now();
                            match executor.execute_frame(&frame_data).await {
                                Ok(()) => {
                                    if let Some(telemetry) = telemetry() {
                                        telemetry.record_frame_processed(execute_start.elapsed());
                                    }
                                    frame_data.mark_executed();

                                    // Update frame data in storage
                                    {
                                        let mut frames = self.frames.write().await;
                                        frames.insert(frame_number, frame_data);
                                    }

                                    // Remove from execution queue
                                    {
                                        let mut queue = self.execution_queue.write().await;
                                        queue.pop_front();
                                    }

                                    // Update current frame
                                    {
                                        let mut current = self.current_frame.write().await;
                                        *current = frame_number + 1;
                                    }

                                    {
                                        let elapsed_ms =
                                            execute_start.elapsed().as_secs_f64() * 1000.0;
                                        let mut stats = self.stats.write().await;
                                        stats.frames_processed += 1;
                                        let processed = stats.frames_processed as f64;
                                        stats.avg_frame_time_ms = if processed <= 1.0 {
                                            elapsed_ms
                                        } else {
                                            ((stats.avg_frame_time_ms * (processed - 1.0))
                                                + elapsed_ms)
                                                / processed
                                        };
                                        let now = NetworkInstant::now();
                                        let mut last_exec = self.last_frame_exec.write().await;
                                        if let Some(prev) = *last_exec {
                                            let delta = now.duration_since(prev).as_secs_f64();
                                            if delta > 0.0 {
                                                stats.current_fps = 1.0 / delta;
                                            }
                                        }
                                        *last_exec = Some(now);
                                    }

                                    executed_count += 1;
                                    trace!("Executed frame {}", frame_number);
                                }
                                Err(e) => {
                                    error!("Failed to execute frame {}: {}", frame_number, e);
                                    executor.handle_frame_error(frame_number, e).await;

                                    // Remove failed frame from queue
                                    {
                                        let mut queue = self.execution_queue.write().await;
                                        queue.pop_front();
                                    }
                                }
                            }
                        } else {
                            warn!("No frame executor set, skipping frame {}", frame_number);
                            break;
                        }
                    } else {
                        // Frame not ready yet
                        break;
                    }
                } else {
                    error!("Frame {} not found for execution", frame_number);

                    // Remove missing frame from queue
                    {
                        let mut queue = self.execution_queue.write().await;
                        queue.pop_front();
                    }
                }
            } else {
                // No frames to execute
                break;
            }

            // Limit executions per update
            if executed_count >= 10 {
                break;
            }
        }

        Ok(())
    }

    /// Clean up old frame data
    async fn cleanup_old_frames(&self) {
        let current_frame = *self.current_frame.read().await;
        let cutoff_frame = current_frame.saturating_sub(self.config.max_frame_history);

        let mut frames = self.frames.write().await;
        frames.retain(|&frame_number, _| frame_number >= cutoff_frame);
    }

    /// Update statistics
    async fn update_statistics(&self) {
        let mut stats = self.stats.write().await;

        // Update pending frames count
        let execution_queue = self.execution_queue.read().await;
        stats.pending_frames = execution_queue.len();
    }

    /// Force frame execution (for timeout situations)
    pub async fn force_execute_frame(&self, frame_number: u32) -> NetworkResult<()> {
        warn!("Force executing frame {} due to timeout", frame_number);

        {
            let mut frames = self.frames.write().await;
            if let Some(frame_data) = frames.get_mut(&frame_number) {
                if !frame_data.is_complete {
                    frame_data.mark_complete();

                    // Add to execution queue
                    let mut queue = self.execution_queue.write().await;
                    queue.push_back(frame_number);
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.frames_skipped += 1;
        }

        Ok(())
    }

    /// Get current frame number
    pub async fn get_current_frame(&self) -> u32 {
        *self.current_frame.read().await
    }

    /// Snapshot command count and checksum for a specific frame, if present.
    pub async fn frame_info(&self, frame_number: u32) -> Option<(u16, u32)> {
        let frames = self.frames.read().await;
        frames.get(&frame_number).map(|frame| {
            let command_count = frame.total_commands.min(u16::MAX as usize) as u16;
            (command_count, frame.checksum)
        })
    }

    /// Current measured frames-per-second value.
    pub async fn current_fps(&self) -> f64 {
        let stats = self.stats.read().await;
        stats.current_fps
    }

    /// Get frame manager statistics
    pub async fn get_stats(&self) -> FrameManagerStats {
        self.stats.read().await.clone()
    }

    /// Get frame history for debugging
    pub async fn get_frame_history(&self, count: usize) -> Vec<FrameStats> {
        let frames = self.frames.read().await;
        let current_frame = *self.current_frame.read().await;

        let mut history = Vec::new();
        for i in 0..count {
            let frame_number = current_frame.saturating_sub(i as u32);
            if let Some(frame_data) = frames.get(&frame_number) {
                history.push(frame_data.get_stats());
            }
        }

        history
    }

    /// Validate frame checksums
    pub async fn validate_frame_checksums(&self) -> NetworkResult<()> {
        if !self.config.enable_validation {
            return Ok(());
        }

        let frames = self.frames.read().await;
        let current_frame = *self.current_frame.read().await;

        // Validate recent frames
        for i in 0..10 {
            let frame_number = current_frame.saturating_sub(i);
            if let Some(frame_data) = frames.get(&frame_number) {
                if frame_data.is_complete && !frame_data.validate_checksum() {
                    return Err(NetworkError::frame_sync(format!(
                        "checksum validation failed for frame {}",
                        frame_number
                    )));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandPayload, GameCommandData};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_frame_data_creation() {
        let frame = FrameData::new(100);
        assert_eq!(frame.frame_number, 100);
        assert_eq!(frame.total_commands, 0);
        assert!(!frame.is_complete);
        assert!(!frame.is_executed);
    }

    #[tokio::test]
    async fn test_frame_command_addition() {
        let mut frame = FrameData::new(100);

        let command = NetCommand::new(
            NetCommandType::GameCommand,
            0,
            100,
            CommandPayload::GameCommand(GameCommandData {
                command_type: 1,
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            }),
        );

        frame.add_command(command).unwrap();
        assert_eq!(frame.total_commands, 1);
        assert_eq!(frame.get_player_commands(0).len(), 1);
    }

    #[tokio::test]
    async fn test_frame_manager_creation() {
        let manager = FrameDataManager::new(10, 2);
        assert_eq!(manager.config.max_frames_ahead, 10);
        assert_eq!(manager.config.min_runahead, 2);

        let current_frame = manager.get_current_frame().await;
        assert_eq!(current_frame, 0);
    }

    #[tokio::test]
    async fn test_frame_checksum_validation() {
        let mut frame = FrameData::new(100);

        // Add some commands
        for player_id in 0..2 {
            let command = NetCommand::new(
                NetCommandType::KeepAlive,
                player_id,
                100,
                CommandPayload::KeepAlive,
            );
            frame.add_command(command).unwrap();
        }

        frame.mark_complete();

        // Checksum should be valid after calculation
        assert!(frame.validate_checksum());

        // Modify payload and checksum should be invalid
        if let Some(commands) = frame.player_commands.get_mut(&0) {
            if let Some(command) = commands.first_mut() {
                command.sequence = command.sequence.wrapping_add(1);
            }
        }
        assert!(!frame.validate_checksum());
    }

    #[test]
    fn test_frame_manager_config() {
        let config = FrameManagerConfig::default();
        assert_eq!(config.target_fps, 30);
        assert!(config.enable_validation);
        assert_eq!(config.max_frame_history, 100);
    }
}
