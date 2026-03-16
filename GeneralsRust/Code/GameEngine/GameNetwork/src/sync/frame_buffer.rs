//! Frame history buffer for replay, anti-cheat, and desync recovery.
//!
//! The frame buffer retains a sliding window of recently executed frames.
//! Each entry records the set of commands that were executed and a CRC
//! checksum of the resulting game state.  This data is used for:
//!
//! - **Replay verification**: third-party tools can download the frame
//!   history and verify that no cheats were used.
//! - **Desync recovery**: when a CRC mismatch is detected, the buffer
//!   provides the data needed to roll back to the last good frame.
//! - **Anti-cheat**: the buffer can be periodically sampled by a server
//!   to verify that clients are producing consistent state.

use crate::error::{NetworkError, NetworkResult};
use crate::network_defs::{FRAME_DATA_LENGTH, FRAMES_TO_KEEP, MAX_FRAMES_AHEAD};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::debug;

/// Default run-ahead buffer size (frames to buffer ahead of current execution).
pub const DEFAULT_RUNAHEAD: u32 = 30;

/// Maximum configurable run-ahead.
pub const MAX_RUNAHEAD: u32 = 60;

/// A single recorded frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameEntry {
    /// Logical frame number (monotonically increasing).
    pub frame_number: u32,
    /// CRC-32 of the game state *after* this frame was executed.
    pub state_crc: u32,
    /// Number of commands executed during this frame.
    pub command_count: u16,
    /// Whether all remote peers have acknowledged this frame.
    pub acked_by_all: bool,
    /// Timestamp (in-game tick) when the frame was executed.
    pub tick: u64,
}

impl FrameEntry {
    /// Create a new frame entry.
    pub fn new(frame_number: u32, state_crc: u32, command_count: u16, tick: u64) -> Self {
        Self {
            frame_number,
            state_crc,
            command_count,
            acked_by_all: false,
            tick,
        }
    }
}

/// Acknowledgment tracking for a single frame.
#[derive(Debug, Clone, Default)]
pub struct FrameAckTracker {
    /// Bitmask of players that have acknowledged this frame.
    /// Bit i corresponds to player index i.
    pub ack_mask: u8,
}

impl FrameAckTracker {
    /// Mark a player as having acknowledged a frame.
    pub fn acknowledge(&mut self, player_index: u8) {
        if player_index < 8 {
            self.ack_mask |= 1 << player_index;
        }
    }

    /// Check if a specific player has acknowledged this frame.
    pub fn is_acknowledged_by(&self, player_index: u8) -> bool {
        if player_index < 8 {
            (self.ack_mask & (1 << player_index)) != 0
        } else {
            false
        }
    }

    /// Check if all expected players have acknowledged this frame.
    pub fn all_acknowledged(&self, num_players: u8) -> bool {
        if num_players == 0 {
            return true;
        }
        // All low `num_players` bits must be set.
        let mask = (1u8 << num_players).saturating_sub(1);
        self.ack_mask & mask == mask
    }
}

/// Frame history buffer.
///
/// Maintains a sliding window of the most recently executed frames for
/// replay, anti-cheat, and desync recovery.  The buffer capacity is
/// configurable between 30 and 60 frames (matching the C++ run-ahead
/// constants).
pub struct FrameBuffer {
    /// Ring buffer of frame entries.
    entries: VecDeque<FrameEntry>,
    /// Acknowledgment tracking per frame (same length as entries).
    ack_trackers: VecDeque<FrameAckTracker>,
    /// Capacity (number of frames to retain).
    capacity: usize,
    /// Current (most recently written) frame number, or None if empty.
    current_frame: Option<u32>,
    /// Total frames written since creation/reset.
    total_frames_written: u64,
}

impl FrameBuffer {
    /// Create a new frame buffer with default capacity.
    ///
    /// Default capacity is `FRAMES_TO_KEEP` matching the C++ constant.
    pub fn new() -> Self {
        Self::with_capacity(FRAMES_TO_KEEP)
    }

    /// Create a frame buffer with a specific capacity.
    ///
    /// `capacity` is clamped to `[30, MAX_RUNAHEAD as usize]`.
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.clamp(DEFAULT_RUNAHEAD as usize, MAX_RUNAHEAD as usize);
        Self {
            entries: VecDeque::with_capacity(capacity),
            ack_trackers: VecDeque::with_capacity(capacity),
            capacity,
            current_frame: None,
            total_frames_written: 0,
        }
    }

    /// Record a newly executed frame.
    ///
    /// If the buffer is full the oldest entry is evicted.
    pub fn record_frame(
        &mut self,
        frame_number: u32,
        state_crc: u32,
        command_count: u16,
        tick: u64,
    ) {
        self.total_frames_written += 1;
        self.current_frame = Some(frame_number);

        let entry = FrameEntry::new(frame_number, state_crc, command_count, tick);
        self.entries.push_back(entry);
        self.ack_trackers.push_back(FrameAckTracker::default());

        // Evict oldest if over capacity.
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
            self.ack_trackers.pop_front();
        }
    }

    /// Acknowledge a frame from a specific player.
    pub fn acknowledge_frame(&mut self, frame_number: u32, player_index: u8) {
        if let Some(pos) = self.find_frame_position(frame_number) {
            self.ack_trackers[pos].acknowledge(player_index);
            // Propagate acked_by_all flag.
            let num_players = self.ack_trackers[pos]
                .ack_mask
                .count_ones() as u8;
            if num_players > 0 && self.ack_trackers[pos].all_acknowledged(num_players) {
                if let Some(entry) = self.entries.get_mut(pos) {
                    entry.acked_by_all = true;
                }
            }
        }
    }

    /// Check whether a frame has been acknowledged by all players.
    pub fn is_frame_fully_acked(&self, frame_number: u32, num_players: u8) -> bool {
        if let Some(pos) = self.find_frame_position(frame_number) {
            self.ack_trackers[pos].all_acknowledged(num_players)
        } else {
            false
        }
    }

    /// Look up a frame entry by frame number.
    ///
    /// Returns `None` if the frame has been evicted from the buffer.
    pub fn get_frame(&self, frame_number: u32) -> Option<&FrameEntry> {
        if let Some(pos) = self.find_frame_position(frame_number) {
            self.entries.get(pos)
        } else {
            None
        }
    }

    /// Look up a mutable frame entry by frame number.
    pub fn get_frame_mut(&mut self, frame_number: u32) -> Option<&mut FrameEntry> {
        if let Some(pos) = self.find_frame_position(frame_number) {
            self.entries.get_mut(pos)
        } else {
            None
        }
    }

    /// Get the most recently recorded frame.
    pub fn latest_frame(&self) -> Option<&FrameEntry> {
        self.entries.back()
    }

    /// Get the earliest (oldest) frame still in the buffer.
    pub fn oldest_frame(&self) -> Option<&FrameEntry> {
        self.entries.front()
    }

    /// Number of frames currently in the buffer.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Buffer capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total frames recorded since creation / last reset.
    pub fn total_frames_written(&self) -> u64 {
        self.total_frames_written
    }

    /// Current frame number (latest written), or 0 if never written.
    pub fn current_frame_number(&self) -> u32 {
        self.current_frame.unwrap_or(0)
    }

    /// Compute a CRC-32 of the given byte slice (used by callers for state hashing).
    pub fn compute_crc(data: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// Clear all frame data and reset counters.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.ack_trackers.clear();
        self.current_frame = None;
        self.total_frames_written = 0;
    }

    /// Resize the buffer capacity. Existing entries beyond the new capacity
    /// are discarded (oldest first).
    pub fn resize(&mut self, new_capacity: usize) {
        let new_capacity = new_capacity.clamp(DEFAULT_RUNAHEAD as usize, MAX_RUNAHEAD as usize);
        self.capacity = new_capacity;
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
            self.ack_trackers.pop_front();
        }
    }

    /// Find the index of a frame in the deque by frame number.
    fn find_frame_position(&self, frame_number: u32) -> Option<usize> {
        // Linear scan is fine for <=60 elements.
        self.entries
            .iter()
            .position(|e| e.frame_number == frame_number)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_buffer_basic() {
        let mut buf = FrameBuffer::with_capacity(5);
        assert!(buf.is_empty());

        for i in 0..3 {
            buf.record_frame(i, i * 100, 1, i as u64);
        }

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.current_frame_number(), 2);
        assert_eq!(buf.total_frames_written(), 3);
    }

    #[test]
    fn test_frame_buffer_eviction() {
        let capacity = DEFAULT_RUNAHEAD as usize; // 30 (minimum clamped)
        let mut buf = FrameBuffer::with_capacity(capacity);

        for i in 0..(capacity as u32 + 3) {
            buf.record_frame(i, i * 10, 1, i as u64);
        }

        assert_eq!(buf.len(), capacity);
        // Oldest frame should be 3 (frames 0..3 evicted).
        assert_eq!(buf.oldest_frame().unwrap().frame_number, 3);
        assert_eq!(buf.latest_frame().unwrap().frame_number, capacity as u32 + 2);
    }

    #[test]
    fn test_frame_buffer_ack() {
        let mut buf = FrameBuffer::with_capacity(10);

        for i in 0..4 {
            buf.record_frame(i, 0, 0, i as u64);
        }

        assert!(!buf.is_frame_fully_acked(0, 2));

        buf.acknowledge_frame(0, 0);
        assert!(!buf.is_frame_fully_acked(0, 2));

        buf.acknowledge_frame(0, 1);
        assert!(buf.is_frame_fully_acked(0, 2));
    }

    #[test]
    fn test_frame_buffer_resize() {
        let big_capacity = DEFAULT_RUNAHEAD as usize + 10; // 40
        let small_capacity = DEFAULT_RUNAHEAD as usize; // 30 (minimum)
        let mut buf = FrameBuffer::with_capacity(big_capacity);
        for i in 0..(big_capacity as u32) {
            buf.record_frame(i, 0, 0, i as u64);
        }
        assert_eq!(buf.len(), big_capacity);

        buf.resize(small_capacity);
        assert_eq!(buf.len(), small_capacity);
        assert_eq!(buf.capacity(), small_capacity);
        let expected_oldest = (big_capacity - small_capacity) as u32;
        assert_eq!(buf.oldest_frame().unwrap().frame_number, expected_oldest);
    }

    #[test]
    fn test_crc_computation() {
        let crc1 = FrameBuffer::compute_crc(b"hello world");
        let crc2 = FrameBuffer::compute_crc(b"hello world");
        let crc3 = FrameBuffer::compute_crc(b"different");
        assert_eq!(crc1, crc2);
        assert_ne!(crc1, crc3);
    }

    #[test]
    fn test_frame_buffer_reset() {
        let mut buf = FrameBuffer::with_capacity(10);
        for i in 0..5 {
            buf.record_frame(i, 0, 0, i as u64);
        }
        buf.reset();
        assert!(buf.is_empty());
        assert_eq!(buf.total_frames_written(), 0);
    }

    #[test]
    fn test_get_frame_by_number() {
        let mut buf = FrameBuffer::with_capacity(10);
        for i in 10..20 {
            buf.record_frame(i, i * 7, 2, i as u64);
        }

        assert!(buf.get_frame(5).is_none()); // evicted / never existed
        assert!(buf.get_frame(10).is_some());
        assert_eq!(buf.get_frame(15).unwrap().state_crc, 15 * 7);
    }
}
