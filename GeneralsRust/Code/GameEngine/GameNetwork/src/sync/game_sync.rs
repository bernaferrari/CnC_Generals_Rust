//! Deterministic lockstep game synchronizer.
//!
//! The `GameSynchronizer` collects player commands, buffers them by frame
//! number, and dispatches them in strict lockstep order.  It produces CRC
//! checksums after each frame so that desynchronization can be detected
//! quickly (the C++ original checks every `NET_CRC_INTERVAL` frames).
//!
//! # Command Flow
//!
//! ```text
//!  Local Input  ──> CommandBuffer ──> [frame N] ──> GameLogic
//!  Remote Peer  ──> CommandBuffer ──> [frame N] ──> GameLogic
//!                                              |
//!                                         state CRC
//!                                              |
//!                                      compare with peers
//! ```

use crate::desync_manager::DesyncManager;
use crate::error::{NetworkError, NetworkResult};
use crate::game_info::NET_CRC_INTERVAL;
use crate::network_defs::{FRAMES_TO_KEEP, FRAME_DATA_LENGTH, MAX_FRAMES_AHEAD, MIN_RUNAHEAD};
use crate::sync::frame_buffer::FrameBuffer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::{mpsc, Mutex, Notify};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Synchronization configuration, matching the C++ constants in
/// `NetworkDefs.h` and `NetworkUtil.cpp`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Fixed timestep in frames per second (C++ default: 30).
    pub fps: u32,
    /// Minimum run-ahead frames between command submission and execution.
    /// Must be >= MIN_RUNAHEAD.
    pub min_runahead: u32,
    /// Maximum run-ahead frames.
    pub max_runahead: u32,
    /// Interval (in frames) between CRC comparisons.
    /// C++ default: NET_CRC_INTERVAL (100).
    pub crc_interval: u32,
    /// Maximum number of consecutive desyncs before recovery.
    pub max_desyncs_before_recovery: u32,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            min_runahead: MIN_RUNAHEAD,
            max_runahead: MAX_FRAMES_AHEAD,
            crc_interval: NET_CRC_INTERVAL as u32,
            max_desyncs_before_recovery: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Net command types for the sync layer
// ---------------------------------------------------------------------------

/// A network command produced by the synchronizer for transport.
///
/// This is the wire-format command that gets sent to peers.  It closely
/// mirrors the C++ `GameMessage` / `NetCommand` structures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetCommand {
    /// Player index (0-7) that issued this command.
    pub player_id: u8,
    /// Frame number this command is destined for.
    pub frame: u32,
    /// Opaque command payload bytes (interpreted by GameLogic).
    pub data: Vec<u8>,
}

impl NetCommand {
    /// Create a new network command.
    pub fn new(player_id: u8, frame: u32, data: Vec<u8>) -> Self {
        Self {
            player_id,
            frame,
            data,
        }
    }
}

// ---------------------------------------------------------------------------
// Command buffer
// ---------------------------------------------------------------------------

/// Per-frame command buffer collecting commands from all players.
#[derive(Debug, Clone, Default)]
pub struct CommandBuffer {
    /// Commands indexed by frame number.
    frames: HashMap<u32, Vec<NetCommand>>,
    /// Current run-ahead target (how many frames ahead we are buffering).
    runahead: u32,
}

impl CommandBuffer {
    /// Create a new command buffer.
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            runahead: MIN_RUNAHEAD,
        }
    }

    /// Insert a command into the buffer.
    pub fn insert(&mut self, cmd: NetCommand) {
        self.frames.entry(cmd.frame).or_default().push(cmd);
    }

    /// Take all commands for a given frame, removing them from the buffer.
    pub fn take_frame_commands(&mut self, frame: u32) -> Vec<NetCommand> {
        self.frames.remove(&frame).unwrap_or_default()
    }

    /// Peek at commands for a given frame without removing them.
    pub fn get_frame_commands(&self, frame: u32) -> &[NetCommand] {
        self.frames.get(&frame).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check whether we have commands for a frame from all expected players.
    pub fn has_frame_ready(&self, frame: u32, num_players: u8) -> bool {
        if let Some(cmds) = self.frames.get(&frame) {
            // Check that at least one command exists per player.
            let mut present = [false; 8];
            for cmd in cmds {
                if (cmd.player_id as usize) < 8 {
                    present[cmd.player_id as usize] = true;
                }
            }
            (0..num_players as usize).all(|i| present[i])
        } else {
            false
        }
    }

    /// Discard all frames older than `oldest_frame`.
    pub fn prune(&mut self, oldest_frame: u32) {
        self.frames.retain(|&f, _| f >= oldest_frame);
    }

    /// Get the lowest frame number currently in the buffer.
    pub fn lowest_frame(&self) -> Option<u32> {
        self.frames.keys().copied().min()
    }

    /// Get the highest frame number currently in the buffer.
    pub fn highest_frame(&self) -> Option<u32> {
        self.frames.keys().copied().max()
    }

    /// Update run-ahead target.
    pub fn set_runahead(&mut self, runahead: u32) {
        self.runahead = runahead;
    }

    /// Get current run-ahead.
    pub fn runahead(&self) -> u32 {
        self.runahead
    }

    /// Clear the entire buffer.
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Number of buffered frames.
    pub fn buffered_frame_count(&self) -> usize {
        self.frames.len()
    }
}

// ---------------------------------------------------------------------------
// Sync state
// ---------------------------------------------------------------------------

/// Overall state of the synchronizer state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Waiting for all players to connect.
    Connecting,
    /// Exchanging initial state / loading.
    Loading,
    /// Actively running the game loop.
    Running,
    /// Paused (e.g. menu open, loading screen).
    Paused,
    /// Desync detected, attempting recovery.
    Recovering,
    /// Game finished / synchronizer stopped.
    Stopped,
}

// ---------------------------------------------------------------------------
// Desync recovery action
// ---------------------------------------------------------------------------

/// Action to take when a desync is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesyncRecoveryAction {
    /// Request the missing frame data from the host.
    RequestFrameResend,
    /// Roll back to the last known-good frame.
    RollbackToLastGood,
    /// Disconnect the desynced player.
    DisconnectPlayer(u8),
    /// End the game (unrecoverable).
    EndGame,
}

// ---------------------------------------------------------------------------
// Sync metrics
// ---------------------------------------------------------------------------

/// Performance and health metrics for the sync system.
#[derive(Debug, Clone, Default)]
pub struct SyncMetrics {
    /// Total frames executed.
    pub frames_executed: u64,
    /// Total commands processed.
    pub commands_processed: u64,
    /// Number of CRC checks performed.
    pub crc_checks: u64,
    /// Number of CRC mismatches detected.
    pub crc_mismatches: u64,
    /// Number of frame resend requests issued.
    pub resend_requests: u64,
    /// Current run-ahead in frames.
    pub current_runahead: u32,
    /// Number of times recovery was triggered.
    pub recovery_count: u64,
}

// ---------------------------------------------------------------------------
// Frame dispatch callback
// ---------------------------------------------------------------------------

/// Trait implemented by the game engine to receive dispatched commands.
///
/// The synchronizer calls `on_frame_commands` once per logical frame
/// with the full set of commands for that frame.
pub trait FrameDispatch {
    /// Called by the synchronizer when a frame's commands are ready for execution.
    ///
    /// Implementations must execute the commands deterministically and return
    /// the CRC-32 of the resulting game state.
    fn on_frame_commands(&mut self, frame: u32, commands: &[NetCommand]) -> u32;

    /// Called when the synchronizer transitions to a new state.
    fn on_state_change(&mut self, _new_state: SyncState) {}
}

// ---------------------------------------------------------------------------
// Game Synchronizer
// ---------------------------------------------------------------------------

/// Deterministic lockstep game synchronizer.
///
/// Collects commands from local input and remote peers, orders them by frame
/// number, and dispatches them to `GameLogic` via the `FrameDispatch` trait.
/// Runs a background task that drives the fixed-timestep game loop.
pub struct GameSynchronizer {
    /// Configuration.
    config: SyncConfig,
    /// Current state.
    state: AtomicSyncState,
    /// Per-player command receiver channels.
    command_inputs: Mutex<HashMap<u8, mpsc::UnboundedReceiver<NetCommand>>>,
    /// Outgoing commands to send to peers (player_id -> sender).
    command_outputs: Mutex<HashMap<u8, mpsc::UnboundedSender<NetCommand>>>,
    /// Command buffer.
    command_buffer: Mutex<CommandBuffer>,
    /// Frame history buffer.
    frame_buffer: Mutex<FrameBuffer>,
    /// Desync manager.
    desync_manager: Mutex<DesyncManager>,
    /// Metrics.
    metrics: Mutex<SyncMetrics>,
    /// Current frame counter.
    current_frame: AtomicU32,
    /// Number of players expected.
    num_players: AtomicU32,
    /// Notifier for the game loop tick.
    tick_notify: Notify,
    /// Whether the game loop is running.
    running: AtomicBool,
}

/// Thread-safe wrapper for `SyncState`.
struct AtomicSyncState(std::sync::atomic::AtomicU8);

impl AtomicSyncState {
    fn new(state: SyncState) -> Self {
        Self(std::sync::atomic::AtomicU8::new(state as u8))
    }

    fn load(&self) -> SyncState {
        match self.0.load(Ordering::Relaxed) {
            0 => SyncState::Connecting,
            1 => SyncState::Loading,
            2 => SyncState::Running,
            3 => SyncState::Paused,
            4 => SyncState::Recovering,
            _ => SyncState::Stopped,
        }
    }

    fn store(&self, state: SyncState) {
        self.0.store(state as u8, Ordering::Relaxed);
    }
}

impl GameSynchronizer {
    /// Create a new game synchronizer with the given configuration.
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            state: AtomicSyncState::new(SyncState::Connecting),
            command_inputs: Mutex::new(HashMap::new()),
            command_outputs: Mutex::new(HashMap::new()),
            command_buffer: Mutex::new(CommandBuffer::new()),
            frame_buffer: Mutex::new(FrameBuffer::new()),
            desync_manager: Mutex::new(DesyncManager::new(
                10, // default max desyncs
            )),
            metrics: Mutex::new(SyncMetrics::default()),
            current_frame: AtomicU32::new(0),
            num_players: AtomicU32::new(0),
            tick_notify: Notify::new(),
            running: AtomicBool::new(false),
        }
    }

    /// Register a player's command input channel.
    ///
    /// The returned sender can be used to feed commands into the synchronizer.
    /// Returns an error if the player slot is already registered.
    pub async fn register_player(
        &self,
        player_id: u8,
    ) -> NetworkResult<mpsc::UnboundedSender<NetCommand>> {
        if player_id as usize >= 8 {
            return Err(NetworkError::player("invalid player index"));
        }

        let mut inputs = self.command_inputs.lock().await;
        if inputs.contains_key(&player_id) {
            return Err(NetworkError::player("player already registered"));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        inputs.insert(player_id, rx);

        self.num_players.fetch_add(1, Ordering::SeqCst);
        info!("Registered player {} in synchronizer", player_id);
        Ok(tx)
    }

    /// Unregister a player (e.g. on disconnect).
    pub async fn unregister_player(&self, player_id: u8) {
        self.command_inputs.lock().await.remove(&player_id);
        self.num_players.fetch_max(0, Ordering::SeqCst);
        info!("Unregistered player {} from synchronizer", player_id);
    }

    /// Submit a local command (e.g. from the game engine's input system).
    ///
    /// The command will be queued for the target frame and forwarded to
    /// all remote peers.
    pub async fn submit_local_command(&self, cmd: NetCommand) -> NetworkResult<()> {
        // Buffer locally.
        self.command_buffer.lock().await.insert(cmd.clone());

        // Forward to all remote peers.
        let outputs = self.command_outputs.lock().await;
        for (&player_id, sender) in outputs.iter() {
            // Don't echo back to the sender.
            if player_id != cmd.player_id {
                if sender.send(cmd.clone()).is_err() {
                    debug!("Failed to forward command to player {}", player_id);
                }
            }
        }

        Ok(())
    }

    /// Receive a remote command (called by the transport layer).
    ///
    /// The command is buffered and will be dispatched on its target frame.
    pub async fn receive_remote_command(&self, cmd: NetCommand) -> NetworkResult<()> {
        // Basic validation.
        if cmd.player_id as usize >= 8 {
            return Err(NetworkError::invalid_command("player index out of range"));
        }
        let current = self.current_frame.load(Ordering::SeqCst);
        let max_frame = current.saturating_add(self.config.max_runahead);
        if cmd.frame > max_frame {
            return Err(NetworkError::invalid_command(format!(
                "command frame {} is too far ahead (current: {}, max: {})",
                cmd.frame, current, max_frame
            )));
        }

        self.command_buffer.lock().await.insert(cmd);
        Ok(())
    }

    /// Connect an output channel for a remote player (commands to send to them).
    pub async fn connect_player_output(
        &self,
        player_id: u8,
        sender: mpsc::UnboundedSender<NetCommand>,
    ) {
        self.command_outputs.lock().await.insert(player_id, sender);
    }

    /// Disconnect an output channel.
    pub async fn disconnect_player_output(&self, player_id: u8) {
        self.command_outputs.lock().await.remove(&player_id);
    }

    /// Transition to a new state.
    pub fn set_state(&self, state: SyncState) {
        let prev = self.state.load();
        self.state.store(state);
        if prev != state {
            info!("Sync state: {:?} -> {:?}", prev, state);
        }
    }

    /// Get the current state.
    pub fn state(&self) -> SyncState {
        self.state.load()
    }

    /// Get the current frame counter.
    pub fn current_frame(&self) -> u32 {
        self.current_frame.load(Ordering::SeqCst)
    }

    /// Get a snapshot of sync metrics.
    pub async fn metrics(&self) -> SyncMetrics {
        self.metrics.lock().await.clone()
    }

    /// Set the number of expected players.
    pub fn set_num_players(&self, count: u32) {
        self.num_players.store(count.min(8), Ordering::SeqCst);
    }

    /// Run one tick of the game loop (blocking, intended for the main loop).
    ///
    /// Returns `Some(frame_commands)` if a frame was dispatched, `None` if
    /// the frame is not yet ready.
    ///
    /// The caller should provide a `FrameDispatch` implementation that
    /// executes the commands and returns the resulting state CRC.
    pub async fn tick<D: FrameDispatch>(
        &self,
        dispatch: &mut D,
    ) -> NetworkResult<Option<(u32, Vec<NetCommand>)>> {
        if self.state.load() != SyncState::Running {
            return Ok(None);
        }

        // Drain incoming commands from all player channels into the buffer.
        self.drain_input_channels().await;

        let current = self.current_frame.load(Ordering::SeqCst);
        let num_players = self.num_players.load(Ordering::SeqCst) as u8;
        let runahead = self.command_buffer.lock().await.runahead();

        // Check if we should try to execute `current` frame.
        // Frame is ready when we have commands from all expected players
        // or the frame has been pending long enough (timeout).
        let target_frame = current;
        let ready = {
            let buf = self.command_buffer.lock().await;
            buf.has_frame_ready(target_frame, num_players)
                || buf.lowest_frame() == Some(target_frame)
        };

        if !ready {
            return Ok(None);
        }

        // Extract commands for this frame.
        let commands = self
            .command_buffer
            .lock()
            .await
            .take_frame_commands(target_frame);
        let command_count = commands.len();

        // Execute through the dispatch handler.
        let state_crc = dispatch.on_frame_commands(target_frame, &commands);

        // Record in frame buffer.
        {
            let mut fb = self.frame_buffer.lock().await;
            fb.record_frame(
                target_frame,
                state_crc,
                command_count as u16,
                target_frame as u64,
            );
        }

        // Update metrics.
        {
            let mut metrics = self.metrics.lock().await;
            metrics.frames_executed += 1;
            metrics.commands_processed += command_count as u64;
            metrics.current_runahead = runahead;
        }

        // Advance frame counter.
        self.current_frame.fetch_add(1, Ordering::SeqCst);

        // Prune old commands.
        let new_current = self.current_frame.load(Ordering::SeqCst);
        {
            let mut buf = self.command_buffer.lock().await;
            buf.prune(new_current.saturating_sub(self.config.max_runahead));
        }

        Ok(Some((target_frame, commands)))
    }

    /// Verify CRC for a frame against a remote peer's CRC.
    ///
    /// Returns `Ok(true)` if they match, `Ok(false)` if they don't (but
    /// we're within tolerance), `Err` if the desync threshold is exceeded.
    pub async fn verify_crc(
        &self,
        frame: u32,
        remote_crc: u32,
        remote_player_id: u8,
    ) -> NetworkResult<bool> {
        let local_crc = {
            let fb = self.frame_buffer.lock().await;
            fb.get_frame(frame).map(|e| e.state_crc).unwrap_or(0)
        };

        if local_crc == remote_crc {
            return Ok(true);
        }

        // CRC mismatch.
        warn!(
            "CRC mismatch at frame {}: local=0x{:08X} remote(player {}) =0x{:08X}",
            frame, local_crc, remote_player_id, remote_crc
        );

        let mut dm = self.desync_manager.lock().await;
        dm.check_frame_crc(frame, local_crc, remote_crc, remote_player_id)?;

        {
            let mut metrics = self.metrics.lock().await;
            metrics.crc_mismatches += 1;
        }

        Ok(false)
    }

    /// Determine the recommended recovery action for the current desync state.
    pub async fn recovery_action(&self) -> DesyncRecoveryAction {
        let dm = self.desync_manager.lock().await;
        if dm.is_in_recovery_mode() {
            // Already in recovery; request frame resend.
            DesyncRecoveryAction::RequestFrameResend
        } else if dm.desync_count() >= self.config.max_desyncs_before_recovery as usize {
            DesyncRecoveryAction::EndGame
        } else {
            DesyncRecoveryAction::RollbackToLastGood
        }
    }

    /// Enter recovery mode: roll back to the last known-good frame.
    pub async fn enter_recovery(&self) -> NetworkResult<u32> {
        let last_good = {
            let dm = self.desync_manager.lock().await;
            dm.last_known_good_frame()
        };

        self.state.store(SyncState::Recovering);
        {
            let mut dm = self.desync_manager.lock().await;
            dm.enter_recovery_mode(last_good);
        }
        {
            let mut metrics = self.metrics.lock().await;
            metrics.recovery_count += 1;
        }

        info!("Entering recovery mode at frame {}", last_good);
        Ok(last_good)
    }

    /// Exit recovery mode after successful resynchronization.
    pub async fn exit_recovery(&self) {
        self.state.store(SyncState::Running);
        let mut dm = self.desync_manager.lock().await;
        dm.exit_recovery_mode();
        info!("Exited recovery mode");
    }

    /// Reset the synchronizer for a new game.
    pub async fn reset(&self) {
        self.state.store(SyncState::Connecting);
        self.current_frame.store(0, Ordering::SeqCst);
        self.num_players.store(0, Ordering::SeqCst);
        self.command_buffer.lock().await.clear();
        self.frame_buffer.lock().await.reset();
        self.desync_manager.lock().await.reset();
        *self.metrics.lock().await = SyncMetrics::default();
        info!("Synchronizer reset");
    }

    /// Get the run-ahead buffer depth (how many frames we are currently ahead).
    pub async fn runahead_depth(&self) -> u32 {
        let buf = self.command_buffer.lock().await;
        let highest = buf.highest_frame().unwrap_or(0);
        let current = self.current_frame.load(Ordering::SeqCst);
        highest.saturating_sub(current)
    }

    /// Adjust the run-ahead target based on network conditions.
    ///
    /// Called periodically by the transport layer when latency information
    /// is available.
    pub async fn adjust_runahead(&self, new_runahead: u32) {
        let clamped = new_runahead.clamp(self.config.min_runahead, self.config.max_runahead);
        self.command_buffer.lock().await.set_runahead(clamped);
        debug!("Run-ahead adjusted to {}", clamped);
    }

    /// Drain all player input channels into the command buffer.
    async fn drain_input_channels(&self) {
        let mut inputs = self.command_inputs.lock().await;
        for (&player_id, rx) in inputs.iter_mut() {
            // Drain all pending commands.
            while let Ok(cmd) = rx.try_recv() {
                self.command_buffer.lock().await.insert(cmd);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDispatch;

    impl FrameDispatch for MockDispatch {
        fn on_frame_commands(&mut self, frame: u32, commands: &[NetCommand]) -> u32 {
            // Simple CRC: just return frame * command_count * 12345.
            let count = commands.len() as u32;
            (frame.wrapping_mul(count)).wrapping_mul(12345)
        }
    }

    fn test_config() -> SyncConfig {
        SyncConfig {
            fps: 30,
            min_runahead: 2,
            max_runahead: 10,
            crc_interval: 5,
            max_desyncs_before_recovery: 5,
        }
    }

    #[tokio::test]
    async fn test_synchronizer_creation() {
        let sync = GameSynchronizer::new(test_config());
        assert_eq!(sync.state(), SyncState::Connecting);
        assert_eq!(sync.current_frame(), 0);
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let sync = GameSynchronizer::new(test_config());

        let tx = sync.register_player(0).await.unwrap();
        drop(tx);
        sync.unregister_player(0).await;

        // Can re-register.
        let _tx2 = sync.register_player(0).await.unwrap();
    }

    #[tokio::test]
    async fn test_register_duplicate_fails() {
        let sync = GameSynchronizer::new(test_config());
        let _tx = sync.register_player(0).await.unwrap();
        assert!(sync.register_player(0).await.is_err());
    }

    #[tokio::test]
    async fn test_command_buffering() {
        let sync = GameSynchronizer::new(test_config());
        sync.set_num_players(2);
        sync.register_player(0).await.unwrap();
        sync.register_player(1).await.unwrap();

        sync.state.store(SyncState::Running);

        // Submit commands for frame 0 from both players.
        sync.submit_local_command(NetCommand::new(0, 0, vec![1, 2, 3]))
            .await
            .unwrap();
        sync.submit_local_command(NetCommand::new(1, 0, vec![4, 5, 6]))
            .await
            .unwrap();

        let mut dispatch = MockDispatch;
        let result = sync.tick(&mut dispatch).await.unwrap();
        assert!(result.is_some());
        let (frame, cmds) = result.unwrap();
        assert_eq!(frame, 0);
        assert_eq!(cmds.len(), 2);
        assert_eq!(sync.current_frame(), 1);
    }

    #[tokio::test]
    async fn test_tick_returns_none_when_not_ready() {
        let sync = GameSynchronizer::new(test_config());
        sync.set_num_players(2);
        sync.state.store(SyncState::Running);

        let mut dispatch = MockDispatch;
        let result = sync.tick(&mut dispatch).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_crc_verification() {
        let sync = GameSynchronizer::new(test_config());
        sync.set_num_players(1);
        sync.register_player(0).await.unwrap();
        sync.state.store(SyncState::Running);

        // Execute a frame to populate the frame buffer.
        sync.submit_local_command(NetCommand::new(0, 0, vec![42]))
            .await
            .unwrap();
        let mut dispatch = MockDispatch;
        sync.tick(&mut dispatch).await.unwrap();

        // Verify with correct CRC should succeed.
        // MockDispatch: frame=0, count=1 => 0 * 1 * 12345 = 0
        let result = sync.verify_crc(0, 0, 0).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_remote_command_too_far_ahead() {
        let sync = GameSynchronizer::new(test_config());
        let result = sync
            .receive_remote_command(NetCommand::new(0, 9999, vec![]))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reset() {
        let sync = GameSynchronizer::new(test_config());
        sync.state.store(SyncState::Running);
        sync.current_frame.store(100, Ordering::SeqCst);

        sync.reset().await;
        assert_eq!(sync.state(), SyncState::Connecting);
        assert_eq!(sync.current_frame(), 0);
    }

    #[tokio::test]
    async fn test_runahead_adjustment() {
        let sync = GameSynchronizer::new(test_config());
        sync.adjust_runahead(5).await;
        assert_eq!(sync.command_buffer.lock().await.runahead(), 5);

        // Clamped to max.
        sync.adjust_runahead(100).await;
        assert_eq!(sync.command_buffer.lock().await.runahead(), 10);

        // Clamped to min.
        sync.adjust_runahead(0).await;
        assert_eq!(sync.command_buffer.lock().await.runahead(), 2);
    }
}
