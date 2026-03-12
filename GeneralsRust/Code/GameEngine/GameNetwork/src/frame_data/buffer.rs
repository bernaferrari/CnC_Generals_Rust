//! Frame data circular buffer
//!
//! This module implements a circular buffer for frame data storage,
//! matching the C++ FrameDataManager's modulo-based approach.
//! The buffer uses frame number modulo FRAME_DATA_LENGTH for indexing.

use super::FrameData;
use crate::commands::NetCommand;
use crate::config;
use crate::error::NetworkResult;
use tracing::{debug, trace, warn};

/// Return type for frame readiness checks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReadyState {
    /// Frame is not ready (still waiting for commands)
    NotReady,
    /// Frame is ready for execution
    Ready,
    /// Frame has too many commands, need to resend
    Resend,
}

/// Circular buffer for frame data using modulo indexing
///
/// This matches the C++ implementation where frame data is stored in a fixed-size
/// array indexed by (frame_number % FRAME_DATA_LENGTH). This allows efficient
/// storage of future frames without unbounded memory growth.
pub struct FrameBuffer {
    /// Fixed-size array of frame data slots
    frames: Vec<FrameData>,
    /// Whether this buffer is for the local player (affects frame command count tracking)
    is_local: bool,
    /// Quit frame number (if quitting)
    quit_frame: Option<u32>,
    /// Whether we are in quitting state
    is_quitting: bool,
}

impl FrameBuffer {
    /// Create a new frame buffer
    ///
    /// # Arguments
    /// * `is_local` - If true, this buffer is for the local player and will automatically
    ///                set frame command count to match received count
    pub fn new(is_local: bool) -> Self {
        let capacity = config::FRAME_DATA_LENGTH;
        let mut frames = Vec::with_capacity(capacity);

        // Initialize all frame slots
        for i in 0..capacity {
            frames.push(FrameData::new(i as u32));
        }

        Self {
            frames,
            is_local,
            quit_frame: None,
            is_quitting: false,
        }
    }

    /// Get the frame index for a given frame number (modulo FRAME_DATA_LENGTH)
    #[inline]
    fn frame_index(&self, frame_number: u32) -> usize {
        (frame_number as usize) % config::FRAME_DATA_LENGTH
    }

    /// Add a command to the appropriate frame
    pub fn add_command(&mut self, command: NetCommand) -> NetworkResult<()> {
        let frame_number = command.execution_frame;
        let index = self.frame_index(frame_number);

        trace!(
            "Adding command type {:?} to frame {} (index {})",
            command.command_type,
            frame_number,
            index
        );

        // Get the frame data
        let frame = &mut self.frames[index];

        // Add the command
        frame.add_command(command)?;

        // If this is the local buffer, automatically update frame command count
        if self.is_local {
            let total = frame.total_commands;
            frame.set_frame_command_count(total as u32);
        }

        Ok(())
    }

    /// Check if all commands are ready for a specific frame
    pub fn check_frame_ready(&self, frame_number: u32) -> FrameReadyState {
        let index = self.frame_index(frame_number);
        let frame = &self.frames[index];

        // Check if this is the correct frame (handle wrap-around)
        // Allow mismatch at FRAME_DATA_LENGTH boundary for circular buffer wraparound
        if frame.frame_number != frame_number && frame_number != config::FRAME_DATA_LENGTH as u32 {
            warn!(
                "Frame mismatch: expected {}, got {}",
                frame_number, frame.frame_number
            );
        }

        // Get expected command count
        let expected = frame.get_frame_command_count();
        let received = frame.total_commands as u32;

        if expected == received {
            FrameReadyState::Ready
        } else if received > expected {
            // Too many commands - possible desync, request resend
            warn!(
                "Frame {} has too many commands: expected {}, got {}",
                frame_number, expected, received
            );
            FrameReadyState::Resend
        } else {
            FrameReadyState::NotReady
        }
    }

    /// Get the command list for a specific frame
    pub fn get_frame_commands(&self, frame_number: u32) -> Vec<NetCommand> {
        let index = self.frame_index(frame_number);
        self.frames[index].get_all_commands_ordered()
    }

    /// Get frame data for a specific frame
    pub fn get_frame(&self, frame_number: u32) -> &FrameData {
        let index = self.frame_index(frame_number);
        &self.frames[index]
    }

    /// Get mutable frame data for a specific frame
    pub fn get_frame_mut(&mut self, frame_number: u32) -> &mut FrameData {
        let index = self.frame_index(frame_number);
        &mut self.frames[index]
    }

    /// Reset a specific frame
    ///
    /// # Arguments
    /// * `frame_number` - Frame to reset
    /// * `is_advancing` - If true, set the frame number to frame + MAX_FRAMES_AHEAD
    ///                    (for circular buffer advancement)
    pub fn reset_frame(&mut self, frame_number: u32, is_advancing: bool) {
        let index = self.frame_index(frame_number);

        // Reset the frame
        self.frames[index] = if is_advancing {
            // When advancing, set to future frame number
            FrameData::new(frame_number + config::MAX_FRAMES_AHEAD)
        } else {
            FrameData::new(frame_number)
        };

        // If local, set command count
        if self.is_local {
            self.frames[index].set_frame_command_count(0);
        }

        debug!(
            "Reset frame {} (index {}), advancing: {}",
            frame_number, index, is_advancing
        );
    }

    /// Set the expected frame command count for a frame
    pub fn set_frame_command_count(&mut self, frame_number: u32, count: u32) {
        let index = self.frame_index(frame_number);
        self.frames[index].set_frame_command_count(count);
    }

    /// Get the expected frame command count for a frame
    pub fn get_frame_command_count(&self, frame_number: u32) -> u32 {
        let index = self.frame_index(frame_number);
        self.frames[index].get_frame_command_count()
    }

    /// Get the actual received command count for a frame
    pub fn get_received_command_count(&self, frame_number: u32) -> u32 {
        let index = self.frame_index(frame_number);
        self.frames[index].total_commands as u32
    }

    /// Zero out frames (set both expected and received counts to 0)
    pub fn zero_frames(&mut self, starting_frame: u32, num_frames: u32) {
        for i in 0..num_frames {
            let frame_number = starting_frame + i;
            let index = self.frame_index(frame_number);
            let frame = &mut self.frames[index];
            frame.total_commands = 0;
            frame.set_frame_command_count(0);
        }

        debug!(
            "Zeroed {} frames starting from frame {}",
            num_frames, starting_frame
        );
    }

    /// Destroy all game messages in all frames
    pub fn destroy_all_messages(&mut self) {
        for frame in &mut self.frames {
            frame.player_commands.clear();
            frame.total_commands = 0;
        }
    }

    /// Set quit frame
    pub fn set_quit_frame(&mut self, frame_number: u32) {
        self.is_quitting = true;
        self.quit_frame = Some(frame_number);
        debug!("Set quit frame to {}", frame_number);
    }

    /// Get quit frame
    pub fn get_quit_frame(&self) -> Option<u32> {
        self.quit_frame
    }

    /// Check if quitting
    pub fn is_quitting(&self) -> bool {
        self.is_quitting
    }

    /// Initialize all frames
    pub fn init(&mut self) {
        for i in 0..self.frames.len() {
            self.frames[i] = FrameData::new(i as u32);
            if self.is_local {
                self.frames[i].set_frame_command_count(0);
            }
        }
        self.is_quitting = false;
        self.quit_frame = None;
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Extended FrameData methods for command count tracking
impl FrameData {
    /// Set the expected frame command count
    ///
    /// This is the total number of commands expected from all players for this frame.
    /// The frame is ready when received count matches expected count.
    pub fn set_frame_command_count(&mut self, count: u32) {
        // Store in checksum field temporarily (we'll add a proper field later if needed)
        self.checksum = count;
    }

    /// Get the expected frame command count
    pub fn get_frame_command_count(&self) -> u32 {
        self.checksum // Using checksum field for now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandPayload, NetCommandType};

    #[test]
    fn test_buffer_creation() {
        let buffer = FrameBuffer::new(false);
        assert_eq!(buffer.frames.len(), config::FRAME_DATA_LENGTH);
        assert!(!buffer.is_local);
        assert!(!buffer.is_quitting());
    }

    #[test]
    fn test_frame_indexing() {
        let buffer = FrameBuffer::new(false);

        // Test modulo indexing
        assert_eq!(buffer.frame_index(0), 0);
        assert_eq!(buffer.frame_index(config::FRAME_DATA_LENGTH as u32), 0);
        assert_eq!(buffer.frame_index(5), 5);
        assert_eq!(buffer.frame_index(config::FRAME_DATA_LENGTH as u32 + 5), 5);
    }

    #[test]
    fn test_add_command() {
        let mut buffer = FrameBuffer::new(false);

        let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive);

        assert!(buffer.add_command(cmd).is_ok());

        let commands = buffer.get_frame_commands(100);
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn test_local_buffer_auto_count() {
        let mut buffer = FrameBuffer::new(true);

        // Add commands to local buffer
        for i in 0..5 {
            let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive)
                .with_sequence(i);
            buffer.add_command(cmd).unwrap();
        }

        // Local buffer should auto-update frame command count
        assert_eq!(buffer.get_frame_command_count(100), 5);
        assert_eq!(buffer.get_received_command_count(100), 5);
    }

    #[test]
    fn test_frame_ready_state() {
        let mut buffer = FrameBuffer::new(false);

        // Set expected count
        buffer.set_frame_command_count(100, 3);

        // Not ready yet
        assert_eq!(buffer.check_frame_ready(100), FrameReadyState::NotReady);

        // Add commands
        for i in 0..3 {
            let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive)
                .with_sequence(i);
            buffer.add_command(cmd).unwrap();
        }

        // Should be ready now
        assert_eq!(buffer.check_frame_ready(100), FrameReadyState::Ready);

        // Add one more (too many)
        let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive)
            .with_sequence(3);
        buffer.add_command(cmd).unwrap();

        // Should need resend
        assert_eq!(buffer.check_frame_ready(100), FrameReadyState::Resend);
    }

    #[test]
    fn test_reset_frame() {
        let mut buffer = FrameBuffer::new(false);

        // Add command
        let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive);
        buffer.add_command(cmd).unwrap();

        // Reset without advancing
        buffer.reset_frame(100, false);
        assert_eq!(buffer.get_received_command_count(100), 0);
        assert_eq!(buffer.get_frame(100).frame_number, 100);

        // Add command again
        let cmd = NetCommand::new(NetCommandType::KeepAlive, 0, 100, CommandPayload::KeepAlive);
        buffer.add_command(cmd).unwrap();

        // Reset with advancing
        buffer.reset_frame(100, true);
        assert_eq!(buffer.get_received_command_count(100), 0);
        assert_eq!(
            buffer.get_frame(100).frame_number,
            100 + config::MAX_FRAMES_AHEAD
        );
    }

    #[test]
    fn test_zero_frames() {
        let mut buffer = FrameBuffer::new(false);

        // Add commands to multiple frames
        for frame_num in 100..105 {
            buffer.set_frame_command_count(frame_num, 10);
            let cmd = NetCommand::new(
                NetCommandType::KeepAlive,
                0,
                frame_num,
                CommandPayload::KeepAlive,
            );
            buffer.add_command(cmd).unwrap();
        }

        // Zero frames 100-104
        buffer.zero_frames(100, 5);

        // Check all are zeroed
        for frame_num in 100..105 {
            assert_eq!(buffer.get_received_command_count(frame_num), 0);
            assert_eq!(buffer.get_frame_command_count(frame_num), 0);
        }
    }

    #[test]
    fn test_quit_frame() {
        let mut buffer = FrameBuffer::new(false);

        assert!(!buffer.is_quitting());
        assert!(buffer.get_quit_frame().is_none());

        buffer.set_quit_frame(500);

        assert!(buffer.is_quitting());
        assert_eq!(buffer.get_quit_frame(), Some(500));
    }

    #[test]
    fn test_circular_buffer_wraparound() {
        let mut buffer = FrameBuffer::new(false);

        // Add commands that wrap around the buffer
        let frame1 = 0;
        let frame2 = config::FRAME_DATA_LENGTH as u32;
        let frame3 = config::FRAME_DATA_LENGTH as u32 * 2;

        // All these frames map to index 0
        assert_eq!(buffer.frame_index(frame1), 0);
        assert_eq!(buffer.frame_index(frame2), 0);
        assert_eq!(buffer.frame_index(frame3), 0);

        // Add to first frame
        let cmd1 = NetCommand::new(
            NetCommandType::KeepAlive,
            0,
            frame1,
            CommandPayload::KeepAlive,
        );
        buffer.add_command(cmd1).unwrap();
        assert_eq!(buffer.get_received_command_count(frame1), 1);

        // Reset to the next frame at the same index (frame1 + FRAME_DATA_LENGTH)
        // When is_advancing = true, it sets frame_number to frame + MAX_FRAMES_AHEAD
        // We want to reset so that frame2 can be added to the same slot
        buffer.reset_frame(frame1, false); // Don't advance, just reset
        buffer.set_frame_command_count(frame2, 0); // Prepare for frame2

        // Manually set the frame number to frame2 at this index
        buffer.get_frame_mut(frame2).frame_number = frame2;

        let cmd2 = NetCommand::new(
            NetCommandType::KeepAlive,
            0,
            frame2,
            CommandPayload::KeepAlive,
        );
        buffer.add_command(cmd2).unwrap();
        assert_eq!(buffer.get_received_command_count(frame2), 1);
    }
}
