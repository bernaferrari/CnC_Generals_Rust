//! Frame synchronization for deterministic gameplay
//!
//! This module provides frame-perfect synchronization for RTS gameplay,
//! ensuring all players execute commands in the same order and at the same time.

use crate::commands::NetCommand;
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Frame synchronization state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Waiting for commands from players
    WaitingForCommands,
    /// Ready to execute frame
    Ready,
    /// Frame execution in progress
    Executing,
    /// Synchronization failed
    Failed,
}

/// Frame data for a single frame
#[derive(Debug, Clone)]
pub struct FrameData {
    /// Frame number
    pub frame_number: u32,
    /// Commands for this frame organized by player
    pub commands: HashMap<u8, Vec<NetCommand>>,
    /// Timestamp when frame was created
    pub timestamp: NetworkInstant,
    /// Whether this frame is ready for execution
    pub ready: bool,
}

impl FrameData {
    /// Create new frame data
    pub fn new(frame_number: u32) -> Self {
        Self {
            frame_number,
            commands: HashMap::new(),
            timestamp: NetworkInstant::now(),
            ready: false,
        }
    }

    /// Add command to this frame
    pub fn add_command(&mut self, player_id: u8, command: NetCommand) {
        self.commands.entry(player_id).or_default().push(command);
    }

    /// Check if frame is ready (has commands from all expected players)
    pub fn check_ready(&mut self, expected_players: &[u8]) -> bool {
        // Frame is ready if we have received commands (or empty command list) from all players
        let ready = expected_players
            .iter()
            .all(|&player_id| self.commands.contains_key(&player_id));

        self.ready = ready;
        ready
    }

    /// Get total number of commands in this frame
    pub fn command_count(&self) -> usize {
        self.commands.values().map(|cmds| cmds.len()).sum()
    }
}

/// Frame synchronizer for deterministic lockstep simulation
pub struct FrameSynchronizer {
    /// Current frame number
    current_frame: Arc<RwLock<u32>>,
    /// Frame buffer organized by frame number
    frame_buffer: Arc<RwLock<BTreeMap<u32, FrameData>>>,
    /// List of expected player IDs
    expected_players: Arc<RwLock<Vec<u8>>>,
    /// Maximum frames to keep in buffer
    max_buffer_frames: u32,
    /// Frame timeout duration
    frame_timeout: Duration,
    /// Synchronization state
    sync_state: Arc<RwLock<SyncState>>,

    // Async coordination
    frame_ready_tx: broadcast::Sender<u32>,

    // Statistics
    frames_processed: Arc<RwLock<u64>>,
    sync_failures: Arc<RwLock<u64>>,
}

impl FrameSynchronizer {
    /// Create a new synchronizer
    pub fn new(max_buffer_frames: u32, frame_timeout_ms: u64) -> Self {
        let (frame_ready_tx, _) = broadcast::channel(100);

        Self {
            current_frame: Arc::new(RwLock::new(0)),
            frame_buffer: Arc::new(RwLock::new(BTreeMap::new())),
            expected_players: Arc::new(RwLock::new(Vec::new())),
            max_buffer_frames,
            frame_timeout: Duration::from_millis(frame_timeout_ms),
            sync_state: Arc::new(RwLock::new(SyncState::WaitingForCommands)),
            frame_ready_tx,
            frames_processed: Arc::new(RwLock::new(0)),
            sync_failures: Arc::new(RwLock::new(0)),
        }
    }

    /// Set expected players for synchronization
    pub async fn set_expected_players(&self, players: Vec<u8>) -> NetworkResult<()> {
        let mut expected = self.expected_players.write().await;
        *expected = players;

        info!("Frame synchronizer configured for players: {:?}", expected);
        Ok(())
    }

    /// Add command to a specific frame
    pub async fn add_command(
        &self,
        frame_number: u32,
        player_id: u8,
        command: NetCommand,
    ) -> NetworkResult<()> {
        let mut buffer = self.frame_buffer.write().await;

        // Create frame data if it doesn't exist
        let frame_data = buffer
            .entry(frame_number)
            .or_insert_with(|| FrameData::new(frame_number));

        // Add command to frame
        frame_data.add_command(player_id, command);

        // Check if frame is now ready
        let expected_players = self.expected_players.read().await;
        if frame_data.check_ready(&expected_players) {
            debug!(
                "Frame {} is ready with {} commands",
                frame_number,
                frame_data.command_count()
            );

            // Notify that frame is ready
            let _ = self.frame_ready_tx.send(frame_number);
        }

        Ok(())
    }

    /// Wait for a specific frame to be ready
    pub async fn wait_for_frame(&self, frame_number: u32) -> NetworkResult<FrameData> {
        // Check if frame is already ready
        {
            let buffer = self.frame_buffer.read().await;
            if let Some(frame_data) = buffer.get(&frame_number) {
                if frame_data.ready {
                    return Ok(frame_data.clone());
                }
            }
        }

        // Wait for frame ready notification with timeout
        let mut frame_ready_rx = self.frame_ready_tx.subscribe();

        let timeout_result = timeout(self.frame_timeout, async {
            loop {
                match frame_ready_rx.recv().await {
                    Ok(ready_frame) if ready_frame == frame_number => {
                        // Frame is ready, retrieve it
                        let buffer = self.frame_buffer.read().await;
                        if let Some(frame_data) = buffer.get(&frame_number) {
                            if frame_data.ready {
                                return Ok(frame_data.clone());
                            }
                        }
                        // Continue waiting if frame is not actually ready
                    }
                    Ok(_) => {
                        // Different frame ready, continue waiting
                        continue;
                    }
                    Err(e) => {
                        return Err(NetworkError::generic(format!(
                            "Frame ready broadcast error: {}",
                            e
                        )));
                    }
                }
            }
        })
        .await;

        match timeout_result {
            Ok(result) => result,
            Err(_) => {
                // Timeout occurred
                warn!("Frame {} synchronization timeout", frame_number);

                {
                    let mut sync_failures = self.sync_failures.write().await;
                    *sync_failures += 1;
                }

                {
                    let mut state = self.sync_state.write().await;
                    *state = SyncState::Failed;
                }

                Err(NetworkError::generic(format!(
                    "Frame {} synchronization timeout",
                    frame_number
                )))
            }
        }
    }

    /// Synchronize frame (wait for all player commands)
    pub async fn sync_frame(&self, frame_number: u32) -> NetworkResult<FrameData> {
        {
            let mut state = self.sync_state.write().await;
            *state = SyncState::WaitingForCommands;
        }

        info!("Synchronizing frame {}", frame_number);

        // Wait for frame to be ready
        let frame_data = self.wait_for_frame(frame_number).await?;

        {
            let mut state = self.sync_state.write().await;
            *state = SyncState::Ready;
        }

        // Update current frame
        {
            let mut current = self.current_frame.write().await;
            *current = frame_number;
        }

        // Update statistics
        {
            let mut processed = self.frames_processed.write().await;
            *processed += 1;
        }

        // Clean up old frames from buffer
        self.cleanup_old_frames(frame_number).await;

        info!(
            "Frame {} synchronized with {} commands",
            frame_number,
            frame_data.command_count()
        );

        Ok(frame_data)
    }

    /// Clean up old frames from buffer
    async fn cleanup_old_frames(&self, current_frame: u32) {
        let mut buffer = self.frame_buffer.write().await;

        // Keep only recent frames
        let cutoff_frame = current_frame.saturating_sub(self.max_buffer_frames);

        // Remove old frames
        let mut to_remove = Vec::new();
        for &frame_num in buffer.keys() {
            if frame_num < cutoff_frame {
                to_remove.push(frame_num);
            }
        }

        for frame_num in to_remove {
            buffer.remove(&frame_num);
        }
    }

    /// Get current frame number
    pub async fn current_frame(&self) -> u32 {
        *self.current_frame.read().await
    }

    /// Get synchronization state
    pub async fn sync_state(&self) -> SyncState {
        *self.sync_state.read().await
    }

    /// Get synchronization statistics
    pub async fn get_stats(&self) -> SyncStats {
        SyncStats {
            current_frame: *self.current_frame.read().await,
            frames_processed: *self.frames_processed.read().await,
            sync_failures: *self.sync_failures.read().await,
            buffer_size: self.frame_buffer.read().await.len(),
            state: *self.sync_state.read().await,
        }
    }

    /// Force advance frame (for recovery from sync failures)
    pub async fn force_advance_frame(&self) -> NetworkResult<()> {
        let mut current = self.current_frame.write().await;
        *current += 1;

        {
            let mut state = self.sync_state.write().await;
            *state = SyncState::WaitingForCommands;
        }

        warn!("Force advanced to frame {}", *current);
        Ok(())
    }
}

/// Synchronization statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// Current frame number
    pub current_frame: u32,
    /// Total frames processed
    pub frames_processed: u64,
    /// Number of synchronization failures
    pub sync_failures: u64,
    /// Current buffer size
    pub buffer_size: usize,
    /// Current synchronization state
    pub state: SyncState,
}

impl Default for FrameSynchronizer {
    fn default() -> Self {
        Self::new(10, 5000) // 10 frame buffer, 5 second timeout
    }
}
