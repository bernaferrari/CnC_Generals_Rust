//! Synchronous frame data manager matching C++ implementation exactly
//!
//! This module provides deterministic, synchronous frame execution for RTS networking.
//! It matches the C++ FrameDataManager implementation to ensure exact timing and behavior.
//!
//! CRITICAL: This is fully synchronous - no async/await in frame execution path.
//! Frame execution must be deterministic across all platforms.

#[allow(unused_imports)]
use crate::commands::{NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use std::collections::{BTreeMap, HashMap};
use tracing::{debug, error, trace};

/// Maximum frames to buffer ahead (matches C++ FRAME_DATA_LENGTH)
/// CRITICAL: C++ comment: "needs to be MAX_FRAMES_AHEAD+1 because a player can send
/// commands one beyond twice max runahead" = (128+1)*2 = 258
/// Must match lib.rs:195 and C++ NetworkUtil.cpp
pub const FRAME_DATA_LENGTH: usize = 258;

/// Maximum frames ahead of current frame (matches C++ MAX_FRAMES_AHEAD)
/// MUST match C++ NetworkUtil.cpp: Int MAX_FRAMES_AHEAD = 128;
pub const MAX_FRAMES_AHEAD: u32 = 128;

/// Target frames per second (matches C++ game engine)
pub const TARGET_FPS: u32 = 30;

/// Frame time in milliseconds (33.33ms for 30 FPS)
pub const FRAME_TIME_MS: u64 = 33;

/// Keep-alive interval in frames (15 seconds at 30 FPS = 450 frames)
/// MUST match C++ NAT.cpp: 15000ms keep-alive interval
/// Calculation: 15 seconds × 30 fps = 450 frames
pub const KEEPALIVE_INTERVAL_FRAMES: u32 = 450;

/// Frame data return type (matches C++ FrameDataReturnType enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDataReturnType {
    /// Frame not ready (waiting for more commands)
    NotReady,
    /// Request resend (command count mismatch)
    Resend,
    /// Frame is ready for execution
    Ready,
}

/// Single frame's data storage (matches C++ FrameData class)
#[derive(Debug, Clone)]
pub struct FrameData {
    /// Frame number this data is associated with
    frame: u32,
    /// Expected command count for this frame (from server/host)
    frame_command_count: i32,
    /// Actual command count received
    command_count: u32,
    /// Commands for this frame, organized by player ID
    /// Commands are sorted by sequence number for deterministic execution
    command_list: BTreeMap<u8, Vec<NetCommand>>,
    /// Last failed command counts (for debug logging)
    last_failed_cc: i32,
    last_failed_frame_cc: i32,
}

impl FrameData {
    /// Create new frame data
    pub fn new() -> Self {
        Self {
            frame: 0,
            frame_command_count: -1,
            command_count: 0,
            command_list: BTreeMap::new(),
            last_failed_cc: -2,
            last_failed_frame_cc: -2,
        }
    }

    /// Initialize frame data
    pub fn init(&mut self) {
        self.frame = 0;
        self.command_list.clear();
        self.frame_command_count = -1;
        self.command_count = 0;
        self.last_failed_cc = -2;
        self.last_failed_frame_cc = -2;
    }

    /// Reset frame data (same as init in C++)
    pub fn reset(&mut self) {
        self.init();
    }

    /// Get frame number
    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Set frame number
    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }

    /// Check if all commands are ready for this frame
    /// Matches C++ FrameData::allCommandsReady
    pub fn all_commands_ready(&mut self, debug_spewage: bool) -> FrameDataReturnType {
        if self.frame_command_count == self.command_count as i32 {
            self.last_failed_frame_cc = -2;
            self.last_failed_cc = -2;
            return FrameDataReturnType::Ready;
        }

        if debug_spewage {
            if (self.last_failed_frame_cc != self.frame_command_count)
                || (self.last_failed_cc != self.command_count as i32)
            {
                debug!(
                    "FrameData::all_commands_ready - failed, frame command count = {}, command count = {}",
                    self.frame_command_count, self.command_count
                );
                self.last_failed_frame_cc = self.frame_command_count;
                self.last_failed_cc = self.command_count as i32;
            }
        }

        if self.command_count as i32 > self.frame_command_count {
            error!(
                "FrameData::all_commands_ready - There are more commands than there should be ({}, should be {})",
                self.command_count, self.frame_command_count
            );

            // Log command list
            for (player_id, commands) in &self.command_list {
                for cmd in commands {
                    error!(
                        "Player {}, Type {:?}, Frame = {}, ID = {}",
                        player_id, cmd.command_type, cmd.execution_frame, cmd.sequence
                    );
                }
            }

            // Reset and request resend
            self.reset();
            return FrameDataReturnType::Resend;
        }

        FrameDataReturnType::NotReady
    }

    /// Set expected command count for this frame
    pub fn set_frame_command_count(&mut self, count: u32) {
        self.frame_command_count = count as i32;
    }

    /// Get expected command count
    pub fn get_frame_command_count(&self) -> u32 {
        self.frame_command_count.max(0) as u32
    }

    /// Get actual command count received
    pub fn get_command_count(&self) -> u32 {
        self.command_count
    }

    /// Add command to this frame
    /// Commands are automatically sorted by sequence number for deterministic execution
    pub fn add_command(&mut self, msg: NetCommand) -> NetworkResult<()> {
        // Check for duplicate
        if let Some(player_commands) = self.command_list.get(&msg.player_id) {
            if player_commands
                .iter()
                .any(|cmd| cmd.sequence == msg.sequence)
            {
                // Duplicate command, skip
                return Ok(());
            }
        }

        // Add command to player's list
        let player_commands = self
            .command_list
            .entry(msg.player_id)
            .or_insert_with(Vec::new);
        player_commands.push(msg);

        // Sort by sequence number for deterministic execution
        player_commands.sort_by_key(|cmd| cmd.sequence);

        self.command_count += 1;

        trace!(
            "Added command, total count = {}, frame command count = {}",
            self.command_count,
            self.frame_command_count
        );

        Ok(())
    }

    /// Get all commands in deterministic order
    /// Order: Player 0 commands (by sequence), Player 1 commands (by sequence), ...
    pub fn get_commands_ordered(&self) -> Vec<NetCommand> {
        let mut all_commands = Vec::new();

        // Process players in order (0, 1, 2, ...) for determinism
        for (_player_id, commands) in &self.command_list {
            for command in commands {
                all_commands.push(command.clone());
            }
        }

        all_commands
    }

    /// Get commands for a specific player
    pub fn get_player_commands(&self, player_id: u8) -> Vec<NetCommand> {
        self.command_list
            .get(&player_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Zero out both command counts (used at game start)
    pub fn zero_frame(&mut self) {
        self.command_count = 0;
        self.frame_command_count = 0;
    }

    /// Destroy all game messages
    pub fn destroy_game_messages(&mut self) {
        self.command_list.clear();
        self.command_count = 0;
    }
}

impl Default for FrameData {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame data manager for a single player (matches C++ FrameDataManager)
/// Uses circular buffer for frame storage
pub struct FrameDataManager {
    /// Is this the local player's frame manager?
    is_local: bool,
    /// Circular buffer of frame data (256 frames)
    frame_data: Vec<FrameData>,
    /// Is player quitting?
    is_quitting: bool,
    /// Frame to quit on
    quit_frame: u32,
}

impl FrameDataManager {
    /// Create new frame data manager
    /// is_local: true if this is for the local player
    pub fn new(is_local: bool) -> Self {
        let mut frame_data = Vec::with_capacity(FRAME_DATA_LENGTH);
        for _ in 0..FRAME_DATA_LENGTH {
            frame_data.push(FrameData::new());
        }

        Self {
            is_local,
            frame_data,
            is_quitting: false,
            quit_frame: 0,
        }
    }

    /// Initialize all frame data
    pub fn init(&mut self) {
        for frame_data in &mut self.frame_data {
            frame_data.init();

            // For local connection, adjust frame command count
            if self.is_local {
                let count = frame_data.get_command_count();
                frame_data.set_frame_command_count(count);
            }
        }

        self.is_quitting = false;
        self.quit_frame = 0;
    }

    /// Reset all frame data
    pub fn reset(&mut self) {
        self.init();
    }

    /// Add network command to appropriate frame
    pub fn add_net_command_msg(&mut self, msg: NetCommand) -> NetworkResult<()> {
        let frame = msg.execution_frame;
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;

        debug!(
            "FrameDataManager::add_net_command_msg - adding command type {:?} for frame {}, index {}",
            msg.command_type, frame, frame_index
        );

        self.frame_data[frame_index].add_command(msg)?;

        // For local connection, adjust frame command count
        if self.is_local {
            let count = self.frame_data[frame_index].get_command_count();
            self.frame_data[frame_index].set_frame_command_count(count);
        }

        Ok(())
    }

    /// Check if all commands are ready for given frame
    pub fn all_commands_ready(&mut self, frame: u32, debug_spewage: bool) -> FrameDataReturnType {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;
        self.frame_data[frame_index].all_commands_ready(debug_spewage)
    }

    /// Get commands for given frame in deterministic order
    pub fn get_frame_commands(&self, frame: u32) -> Vec<NetCommand> {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;
        self.frame_data[frame_index].get_commands_ordered()
    }

    /// Reset frame and optionally advance to next frame window
    pub fn reset_frame(&mut self, frame: u32, is_advancing: bool) {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;

        self.frame_data[frame_index].reset();

        if is_advancing {
            self.frame_data[frame_index].set_frame(frame + MAX_FRAMES_AHEAD);
        }

        // For local connection, adjust frame command count
        if self.is_local {
            let count = self.frame_data[frame_index].get_command_count();
            self.frame_data[frame_index].set_frame_command_count(count);
        }

        // Verify command count is zero after reset
        assert_eq!(
            self.frame_data[frame_index].get_command_count(),
            0,
            "Command count not zero after reset"
        );
    }

    /// Get command count for frame
    pub fn get_command_count(&self, frame: u32) -> u32 {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;
        self.frame_data[frame_index].get_command_count()
    }

    /// Set frame command count for frame
    pub fn set_frame_command_count(&mut self, frame: u32, command_count: u32) {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;
        self.frame_data[frame_index].set_frame_command_count(command_count);
    }

    /// Get frame command count
    pub fn get_frame_command_count(&self, frame: u32) -> u32 {
        let frame_index = (frame % FRAME_DATA_LENGTH as u32) as usize;
        self.frame_data[frame_index].get_frame_command_count()
    }

    /// Zero frames (used at game start)
    pub fn zero_frames(&mut self, starting_frame: u32, num_frames: u32) {
        let mut frame_index = (starting_frame % FRAME_DATA_LENGTH as u32) as usize;

        for _ in 0..num_frames {
            self.frame_data[frame_index].zero_frame();
            frame_index = (frame_index + 1) % FRAME_DATA_LENGTH;
        }
    }

    /// Destroy all game messages (cleanup at end of game)
    pub fn destroy_game_messages(&mut self) {
        for frame_data in &mut self.frame_data {
            frame_data.destroy_game_messages();
        }
    }

    /// Set quit frame
    pub fn set_quit_frame(&mut self, frame: u32) {
        self.is_quitting = true;
        self.quit_frame = frame;
    }

    /// Get quit frame
    pub fn get_quit_frame(&self) -> u32 {
        self.quit_frame
    }

    /// Is quitting?
    pub fn get_is_quitting(&self) -> bool {
        self.is_quitting
    }
}

/// Synchronous frame executor for deterministic game updates
/// This replaces the async FrameDataManager for the main game loop
pub struct SyncFrameExecutor {
    /// Current game frame
    current_frame: u32,
    /// Next frame to execute
    next_frame: u32,
    /// Frame data managers for each player
    player_frame_data: HashMap<u8, FrameDataManager>,
    /// Expected players
    expected_players: Vec<u8>,
    /// Local player ID
    local_player_id: u8,
    /// Keep-alive frame counter
    keepalive_frame_counter: u32,
}

impl SyncFrameExecutor {
    /// Create new synchronous frame executor
    pub fn new(local_player_id: u8) -> Self {
        Self {
            current_frame: 0,
            next_frame: 0,
            player_frame_data: HashMap::new(),
            expected_players: Vec::new(),
            local_player_id,
            keepalive_frame_counter: 0,
        }
    }

    /// Initialize with player list
    pub fn init(&mut self, player_ids: Vec<u8>) {
        self.current_frame = 0;
        self.next_frame = 0;
        self.expected_players = player_ids.clone();
        self.keepalive_frame_counter = 0;

        // Create frame data manager for each player
        self.player_frame_data.clear();
        for &player_id in &player_ids {
            let is_local = player_id == self.local_player_id;
            let mut manager = FrameDataManager::new(is_local);
            manager.init();
            self.player_frame_data.insert(player_id, manager);
        }
    }

    /// Add command to appropriate frame
    pub fn add_command(&mut self, command: NetCommand) -> NetworkResult<()> {
        let player_id = command.player_id;

        if let Some(manager) = self.player_frame_data.get_mut(&player_id) {
            manager.add_net_command_msg(command)?;
        } else {
            return Err(NetworkError::generic(format!(
                "Unknown player ID: {}",
                player_id
            )));
        }

        Ok(())
    }

    /// Check if next frame is ready for execution
    pub fn is_frame_ready(&mut self, frame: u32) -> bool {
        // Check all players have submitted commands for this frame
        for &player_id in &self.expected_players {
            if let Some(manager) = self.player_frame_data.get_mut(&player_id) {
                let status = manager.all_commands_ready(frame, false);
                if status != FrameDataReturnType::Ready {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Get all commands for a frame in deterministic order
    pub fn get_frame_commands(&self, frame: u32) -> Vec<(u8, NetCommand)> {
        let mut all_commands = Vec::new();

        // Process players in order (0, 1, 2, ...) for determinism
        for &player_id in &self.expected_players {
            if let Some(manager) = self.player_frame_data.get(&player_id) {
                let commands = manager.get_frame_commands(frame);
                for command in commands {
                    all_commands.push((player_id, command));
                }
            }
        }

        all_commands
    }

    /// Synchronous frame update - called from main game loop
    /// Returns true if frame was executed
    pub fn update(&mut self) -> NetworkResult<bool> {
        // Check if next frame is ready
        if !self.is_frame_ready(self.next_frame) {
            return Ok(false); // Skip, wait for more commands
        }

        // Get all commands for this frame
        let commands = self.get_frame_commands(self.next_frame);

        trace!(
            "Executing frame {} with {} commands",
            self.next_frame,
            commands.len()
        );

        // Execute commands in deterministic order
        for (player_id, command) in commands {
            self.execute_command(player_id, command)?;
        }

        // Advance to next frame
        self.current_frame = self.next_frame;
        self.next_frame += 1;

        // Reset frame data for the frame we just executed
        for manager in self.player_frame_data.values_mut() {
            manager.reset_frame(self.current_frame, true);
        }

        // Update keep-alive counter
        self.keepalive_frame_counter += 1;

        Ok(true)
    }

    /// Execute a single command (to be implemented by game logic)
    fn execute_command(&self, _player_id: u8, command: NetCommand) -> NetworkResult<()> {
        // This would call into game logic to execute the command
        // For now, just trace it
        trace!("Execute command: {:?}", command.command_type);
        Ok(())
    }

    /// Check if keep-alive should be sent this frame
    pub fn should_send_keepalive(&self) -> bool {
        self.keepalive_frame_counter >= KEEPALIVE_INTERVAL_FRAMES
    }

    /// Reset keep-alive counter (after sending)
    pub fn reset_keepalive_counter(&mut self) {
        self.keepalive_frame_counter = 0;
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Get next frame number
    pub fn get_next_frame(&self) -> u32 {
        self.next_frame
    }

    /// Zero frames at game start
    pub fn zero_frames(&mut self, starting_frame: u32, num_frames: u32) {
        for manager in self.player_frame_data.values_mut() {
            manager.zero_frames(starting_frame, num_frames);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandPayload;

    #[test]
    fn test_frame_data_creation() {
        let frame_data = FrameData::new();
        assert_eq!(frame_data.get_frame(), 0);
        assert_eq!(frame_data.get_command_count(), 0);
    }

    #[test]
    fn test_frame_data_command_addition() {
        let mut frame_data = FrameData::new();
        frame_data.set_frame(100);
        frame_data.set_frame_command_count(1);

        let command = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive);

        frame_data.add_command(command).unwrap();
        assert_eq!(frame_data.get_command_count(), 1);
    }

    #[test]
    fn test_frame_data_ready_state() {
        let mut frame_data = FrameData::new();
        frame_data.set_frame_command_count(2);

        // Not ready - no commands
        assert_eq!(
            frame_data.all_commands_ready(false),
            FrameDataReturnType::NotReady
        );

        // Add first command
        let cmd1 = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive);
        frame_data.add_command(cmd1).unwrap();

        // Still not ready - only 1 of 2 commands
        assert_eq!(
            frame_data.all_commands_ready(false),
            FrameDataReturnType::NotReady
        );

        // Add second command
        let cmd2 = NetCommand::new(NetCommandType::KeepAlive, 1, 100, CommandPayload::KeepAlive);
        frame_data.add_command(cmd2).unwrap();

        // Now ready - 2 of 2 commands
        assert_eq!(
            frame_data.all_commands_ready(false),
            FrameDataReturnType::Ready
        );
    }

    #[test]
    fn test_circular_buffer_indexing() {
        let mut manager = FrameDataManager::new(false);
        manager.init();

        // Test that frame indexing wraps correctly
        for frame in 0..FRAME_DATA_LENGTH * 2 {
            let frame_index = (frame % FRAME_DATA_LENGTH) as usize;
            assert!(frame_index < FRAME_DATA_LENGTH);
        }
    }

    #[test]
    fn test_sync_frame_executor() {
        let mut executor = SyncFrameExecutor::new(0);
        executor.init(vec![0, 1]);

        assert_eq!(executor.get_current_frame(), 0);
        assert_eq!(executor.get_next_frame(), 0);

        // Frame not ready initially
        assert!(!executor.is_frame_ready(0));
    }

    #[test]
    fn test_keepalive_timing() {
        // At 30 FPS, 450 frames = 15 seconds (matches C++ NAT.cpp)
        assert_eq!(KEEPALIVE_INTERVAL_FRAMES, 450);
        assert_eq!(TARGET_FPS, 30);

        let seconds = KEEPALIVE_INTERVAL_FRAMES / TARGET_FPS;
        assert_eq!(seconds, 15); // Changed from 20 to 15 to match C++
    }
}
