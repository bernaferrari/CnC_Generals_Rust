//! Frame Data Manager for Frame Synchronization and Runahead Tracking
//!
//! This module implements deterministic frame-based synchronization matching the C++
//! networking layer. It manages frame buffers, validates runahead bounds, tracks CRC
//! checksums, and maintains frame history for debugging and rollback scenarios.
//!
//! # Architecture
//!
//! The frame manager operates on a circular buffer model where:
//! - Commands arrive for future frames (up to MAX_FRAMES_AHEAD)
//! - Frames execute in strict deterministic order
//! - Runahead is dynamically adjusted based on network conditions
//! - CRC validation ensures all clients stay synchronized
//!
//! # Example
//!
//! ```no_run
//! use game_network::frame_manager::{FrameDataManager, FrameData};
//! use game_network::commands::NetCommand;
//!
//! let mut manager = FrameDataManager::new(10, 2);
//! // Add commands to frames
//! // Advance through frames
//! // Validate synchronization
//! ```

pub mod duplicate_detector;
pub mod reorder_buffer;
pub mod resend_expiry;

use crate::commands::NetCommand;
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use duplicate_detector::FrameDuplicateDetector;
use reorder_buffer::FrameReorderBuffer;
use resend_expiry::{ResendExpiryConfig, ResendExpiryManager};
use std::collections::VecDeque;

/// Maximum frames ahead that can be buffered for commands (from C++ NetworkDefs.h)
/// MUST match C++ NetworkUtil.cpp: Int MAX_FRAMES_AHEAD = 128;
pub const MAX_FRAMES_AHEAD: u32 = 128;

/// Minimum run-ahead frames to maintain between command submission and execution
/// MUST match C++ NetworkUtil.cpp: Int MIN_RUNAHEAD = 10;
pub const MIN_RUNAHEAD: u32 = 10;

/// Frame data buffer length (circular buffer size)
/// CRITICAL: C++ comment explains: "needs to be MAX_FRAMES_AHEAD+1 because a player can send
/// commands one beyond twice max runahead"
/// MUST match C++ NetworkUtil.cpp: Int FRAME_DATA_LENGTH = (128+1)*2 = 258;
pub const FRAME_DATA_LENGTH: u32 = 258;

/// Number of frames to keep in history for debugging and rollback
/// MUST match C++ NetworkUtil.cpp: Int FRAMES_TO_KEEP = (128/2) + 1 = 65;
pub const FRAMES_TO_KEEP: u32 = 65;

/// Frame data containing all commands for a specific game frame
#[derive(Clone, Debug)]
pub struct FrameData {
    /// Frame number this data represents
    pub frame_number: u32,

    /// All commands scheduled for this frame
    pub commands: Vec<NetCommand>,

    /// CRC checksum for frame validation
    pub crc: u32,

    /// Whether this frame has received all expected commands
    pub is_complete: bool,

    /// Timestamp when this frame data was first created
    pub received_at: NetworkInstant,
}

impl FrameData {
    /// Create a new empty frame data
    pub fn new(frame_number: u32) -> Self {
        Self {
            frame_number,
            commands: Vec::new(),
            crc: 0,
            is_complete: false,
            received_at: NetworkInstant::now(),
        }
    }

    /// Add a command to this frame
    pub fn add_command(&mut self, command: NetCommand) {
        self.commands.push(command);
        // Sort by player ID then sequence to ensure deterministic execution order
        self.commands.sort_by(|a, b| {
            a.player_id
                .cmp(&b.player_id)
                .then_with(|| a.sequence.cmp(&b.sequence))
        });
    }

    /// Calculate CRC for this frame's commands
    pub fn calculate_crc(&mut self) {
        let mut data = Vec::new();

        // Add frame number
        data.extend_from_slice(&self.frame_number.to_le_bytes());

        // Add each command in deterministic order
        for cmd in &self.commands {
            data.push(cmd.command_type as u8);
            data.push(cmd.player_id);
            data.extend_from_slice(&cmd.sequence.to_le_bytes());
            data.extend_from_slice(&cmd.execution_frame.to_le_bytes());
        }

        self.crc = crc32fast::hash(&data);
    }

    /// Mark this frame as complete (all expected commands received)
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
        self.calculate_crc();
    }

    /// Get the age of this frame data
    pub fn age(&self) -> std::time::Duration {
        self.received_at.elapsed()
    }
}

/// Frame Data Manager handles frame synchronization and runahead tracking
#[derive(Debug)]
pub struct FrameDataManager {
    /// Circular buffer of frame data (ring buffer)
    frames: VecDeque<FrameData>,

    /// Current execution frame number
    current_frame: u32,

    /// Maximum frames ahead allowed
    max_ahead: u32,

    /// Minimum runahead frames required
    min_runahead: u32,

    /// History of runahead values for adaptive adjustment
    runahead_history: Vec<u32>,

    /// Last frame where a desync was detected
    last_desync_frame: Option<u32>,

    /// Frame history for debugging (limited to FRAMES_TO_KEEP)
    frame_history: VecDeque<FrameData>,

    /// Duplicate frame detector
    duplicate_detector: FrameDuplicateDetector,

    /// Count of duplicate frames detected
    duplicates_detected: usize,

    /// Reorder buffer for handling out-of-order frame arrivals
    reorder_buffer: FrameReorderBuffer,

    /// Maximum number of frames buffered in reorder buffer (for stats)
    max_buffered_frames: usize,

    /// Resend expiry manager for automatic cleanup
    resend_expiry: ResendExpiryManager,

    /// Frame counter for cleanup scheduling
    frame_counter: u32,
}

impl FrameDataManager {
    /// Create a new frame data manager
    ///
    /// # Arguments
    ///
    /// * `max_ahead` - Maximum frames that can be buffered ahead
    /// * `min_runahead` - Minimum runahead frames to maintain
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::FrameDataManager;
    ///
    /// let manager = FrameDataManager::new(10, 2);
    /// ```
    pub fn new(max_ahead: u32, min_runahead: u32) -> Self {
        let max_ahead = max_ahead.max(1);
        let min_runahead = min_runahead.max(1).min(max_ahead);

        Self {
            frames: VecDeque::with_capacity((max_ahead + 3) as usize),
            current_frame: 0,
            max_ahead,
            min_runahead,
            runahead_history: Vec::with_capacity(10),
            last_desync_frame: None,
            frame_history: VecDeque::with_capacity(FRAMES_TO_KEEP as usize),
            duplicate_detector: FrameDuplicateDetector::new(FRAMES_TO_KEEP as usize),
            duplicates_detected: 0,
            reorder_buffer: FrameReorderBuffer::new(),
            max_buffered_frames: 0,
            resend_expiry: ResendExpiryManager::default(),
            frame_counter: 0,
        }
    }

    /// Create a new frame data manager with custom resend expiry configuration
    ///
    /// # Arguments
    ///
    /// * `max_ahead` - Maximum frames that can be buffered ahead
    /// * `min_runahead` - Minimum runahead frames to maintain
    /// * `resend_config` - Configuration for resend expiry management
    ///
    /// # Example
    ///
    /// ```no_run
    /// use game_network::frame_manager::{FrameDataManager, resend_expiry::ResendExpiryConfig};
    ///
    /// let config = ResendExpiryConfig::default();
    /// let manager = FrameDataManager::with_resend_config(10, 2, config);
    /// ```
    pub fn with_resend_config(
        max_ahead: u32,
        min_runahead: u32,
        resend_config: ResendExpiryConfig,
    ) -> Self {
        let max_ahead = max_ahead.max(1);
        let min_runahead = min_runahead.max(1).min(max_ahead);

        Self {
            frames: VecDeque::with_capacity((max_ahead + 3) as usize),
            current_frame: 0,
            max_ahead,
            min_runahead,
            runahead_history: Vec::with_capacity(10),
            last_desync_frame: None,
            frame_history: VecDeque::with_capacity(FRAMES_TO_KEEP as usize),
            duplicate_detector: FrameDuplicateDetector::new(FRAMES_TO_KEEP as usize),
            duplicates_detected: 0,
            reorder_buffer: FrameReorderBuffer::new(),
            max_buffered_frames: 0,
            resend_expiry: ResendExpiryManager::new(resend_config),
            frame_counter: 0,
        }
    }

    /// Add a command to the appropriate frame
    ///
    /// # Arguments
    ///
    /// * `frame` - Target frame number
    /// * `command` - Network command to add
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Frame is in the past
    /// - Frame exceeds runahead limit
    /// - Command is invalid
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # use game_network::commands::{NetCommand, NetCommandType, CommandPayload};
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// let command = NetCommand::keep_alive(0);
    /// manager.add_command(10, command)?;
    /// # Ok::<(), game_network::error::NetworkError>(())
    /// ```
    pub fn add_command(&mut self, frame: u32, command: NetCommand) -> NetworkResult<()> {
        // Validate frame is not in the past
        if frame < self.current_frame {
            return Err(NetworkError::frame_sync(format!(
                "Cannot add command to past frame {} (current: {})",
                frame, self.current_frame
            )));
        }

        // Validate frame doesn't exceed maximum ahead
        if frame > self.current_frame + self.max_ahead {
            return Err(NetworkError::frame_sync(format!(
                "Frame {} exceeds max runahead (current: {}, max_ahead: {})",
                frame, self.current_frame, self.max_ahead
            )));
        }

        // Check if this is a potentially duplicate frame
        // We check if the frame exists and has a CRC calculated
        let is_duplicate =
            if let Some(existing_frame) = self.frames.iter().find(|f| f.frame_number == frame) {
                if existing_frame.crc != 0 {
                    // Frame has CRC, check if it's a duplicate
                    if self
                        .duplicate_detector
                        .is_duplicate(frame, existing_frame.crc)
                    {
                        Some(existing_frame.crc)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

        if let Some(crc) = is_duplicate {
            self.duplicates_detected += 1;
            #[cfg(feature = "metrics")]
            tracing::warn!(
                "Duplicate frame detected: frame={}, crc={:08X}, duplicates_total={}",
                frame,
                crc,
                self.duplicates_detected
            );
            return Ok(()); // Silently ignore duplicate
        }

        // Find or create the frame data
        let frame_data = self.get_or_create_frame_mut(frame);
        frame_data.add_command(command);

        Ok(())
    }

    /// Get frame data for a specific frame number
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame number to retrieve
    ///
    /// # Returns
    ///
    /// Reference to frame data if it exists, None otherwise
    pub fn get_frame(&self, frame: u32) -> Option<&FrameData> {
        self.frames.iter().find(|f| f.frame_number == frame)
    }

    /// Get mutable frame data or create it if it doesn't exist
    fn get_or_create_frame_mut(&mut self, frame: u32) -> &mut FrameData {
        // Check if frame already exists
        if let Some(pos) = self.frames.iter().position(|f| f.frame_number == frame) {
            return &mut self.frames[pos];
        }

        // Create new frame
        let new_frame = FrameData::new(frame);
        self.frames.push_back(new_frame);

        // Sort to maintain order
        self.frames
            .make_contiguous()
            .sort_by_key(|f| f.frame_number);

        // Find the newly inserted frame
        let pos = self
            .frames
            .iter()
            .position(|f| f.frame_number == frame)
            .unwrap();
        &mut self.frames[pos]
    }

    /// Advance to the next frame
    ///
    /// This should be called after the current frame has been executed.
    /// It moves old frames to history and advances the current frame counter.
    /// Also performs automatic cleanup of expired resend requests.
    ///
    /// # Errors
    ///
    /// Returns error if runahead constraints are violated
    pub fn advance_frame(&mut self) -> NetworkResult<()> {
        // Check if we can safely advance
        if !self.is_valid_runahead() {
            return Err(NetworkError::frame_sync(format!(
                "Cannot advance: runahead violation (current: {}, min: {})",
                self.get_runahead(),
                self.min_runahead
            )));
        }

        // Move current frame to history if it exists
        if let Some(pos) = self
            .frames
            .iter()
            .position(|f| f.frame_number == self.current_frame)
        {
            let old_frame = self.frames.remove(pos).unwrap();
            self.add_to_history(old_frame);
        }

        // Advance frame counter
        self.current_frame = self.current_frame.wrapping_add(1);
        self.frame_counter = self.frame_counter.wrapping_add(1);

        // Record runahead for adaptive adjustment
        let current_runahead = self.get_runahead();
        self.runahead_history.push(current_runahead);
        if self.runahead_history.len() > 100 {
            self.runahead_history.remove(0);
        }

        // Automatic cleanup of expired resend requests at scheduled intervals
        if self.frame_counter % self.resend_expiry.cleanup_interval() == 0 {
            let removed = self.resend_expiry.cleanup_expired();
            #[cfg(feature = "metrics")]
            if removed > 0 {
                tracing::debug!(
                    "Auto-cleanup: removed {} expired resend requests at frame {}",
                    removed,
                    self.current_frame
                );
            }
        }

        Ok(())
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Check if a frame is ready to execute
    ///
    /// A frame is ready if:
    /// - It exists in the buffer
    /// - It is marked as complete
    /// - It is the current frame
    pub fn can_execute_frame(&self, frame: u32) -> bool {
        if frame != self.current_frame {
            return false;
        }

        self.frames
            .iter()
            .find(|f| f.frame_number == frame)
            .map(|f| f.is_complete)
            .unwrap_or(false)
    }

    /// Get current runahead (number of frames buffered ahead)
    pub fn get_runahead(&self) -> u32 {
        self.frames
            .iter()
            .map(|f| f.frame_number)
            .max()
            .map(|max_frame| max_frame.saturating_sub(self.current_frame))
            .unwrap_or(0)
    }

    /// Validate that runahead is within acceptable bounds
    pub fn is_valid_runahead(&self) -> bool {
        let runahead = self.get_runahead();
        runahead >= self.min_runahead && runahead <= self.max_ahead
    }

    /// Validate frame CRC against expected value
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame number to validate
    /// * `expected_crc` - Expected CRC value
    ///
    /// # Returns
    ///
    /// true if CRC matches, false otherwise
    pub fn validate_frame_crc(&mut self, frame: u32, expected_crc: u32) -> bool {
        if let Some(frame_data) = self.frames.iter_mut().find(|f| f.frame_number == frame) {
            // Recalculate CRC if not already calculated
            if frame_data.crc == 0 {
                frame_data.calculate_crc();
            }

            let valid = frame_data.crc == expected_crc;
            if !valid {
                self.last_desync_frame = Some(frame);
            }
            valid
        } else {
            false
        }
    }

    /// Mark a frame as complete and register it with the duplicate detector
    ///
    /// This should be called when all expected commands for a frame have been received.
    /// It calculates the CRC and registers the frame with the duplicate detector.
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame number to mark as complete
    ///
    /// # Returns
    ///
    /// true if the frame was successfully marked complete, false if frame not found
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// manager.mark_frame_complete(5);
    /// ```
    pub fn mark_frame_complete(&mut self, frame: u32) -> bool {
        if let Some(frame_data) = self.frames.iter_mut().find(|f| f.frame_number == frame) {
            frame_data.mark_complete();
            // Register with duplicate detector
            self.duplicate_detector.add_frame(frame, frame_data.crc);
            true
        } else {
            false
        }
    }

    /// Get the number of duplicate frames detected
    ///
    /// # Returns
    ///
    /// Total count of duplicate frames detected since manager creation
    pub fn duplicates_detected(&self) -> usize {
        self.duplicates_detected
    }

    /// Get recent frame history
    ///
    /// # Arguments
    ///
    /// * `count` - Number of historical frames to retrieve
    ///
    /// # Returns
    ///
    /// Vector of historical frame data (most recent first)
    pub fn get_frame_history(&self, count: usize) -> Vec<FrameData> {
        self.frame_history
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Clean up old frames from the buffer
    ///
    /// Removes frames that are too far in the past and no longer needed.
    /// This prevents unbounded memory growth.
    pub fn cleanup_old_frames(&mut self) {
        let cutoff_frame = self.current_frame.saturating_sub(self.max_ahead);

        // Remove old frames from main buffer (they should already be in history)
        self.frames.retain(|f| f.frame_number >= cutoff_frame);

        // Limit history size
        while self.frame_history.len() > FRAMES_TO_KEEP as usize {
            self.frame_history.pop_front();
        }
    }

    /// Add a frame to the history buffer
    fn add_to_history(&mut self, mut frame: FrameData) {
        // Ensure CRC is calculated before archiving
        if frame.crc == 0 {
            frame.calculate_crc();
        }

        self.frame_history.push_back(frame);

        // Maintain history size limit
        if self.frame_history.len() > FRAMES_TO_KEEP as usize {
            self.frame_history.pop_front();
        }
    }

    /// Get the last frame where a desync was detected
    pub fn last_desync_frame(&self) -> Option<u32> {
        self.last_desync_frame
    }

    /// Process a complete frame through the reorder buffer
    ///
    /// This method should be used when receiving complete frames from the network.
    /// It uses the reorder buffer to handle out-of-order arrivals.
    ///
    /// # Arguments
    ///
    /// * `frame_data` - Complete frame data received from network
    ///
    /// # Returns
    ///
    /// Vector of frames ready to be processed (may be empty if buffered)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::{FrameDataManager, FrameData};
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// let frame = FrameData::new(5);
    /// let ready_frames = manager.process_frame(frame);
    /// for frame in ready_frames {
    ///     // Process frame
    /// }
    /// ```
    pub fn process_frame(&mut self, frame_data: FrameData) -> Vec<FrameData> {
        // Update max buffered frames stat before processing
        let current_buffered = self.reorder_buffer.len();
        if current_buffered > self.max_buffered_frames {
            self.max_buffered_frames = current_buffered;
        }

        // Use reorder buffer to handle out-of-order arrivals
        match self.reorder_buffer.buffer_frame(frame_data) {
            Some(frames) => {
                // Frames are ready to be delivered
                // Add them to the internal frame buffer
                for frame in &frames {
                    // Only add non-empty frames (empty vec indicates duplicate/dropped)
                    if frame.commands.is_empty() {
                        continue;
                    }

                    // Add frame to internal buffer if not already present
                    if !self
                        .frames
                        .iter()
                        .any(|f| f.frame_number == frame.frame_number)
                    {
                        self.frames.push_back(frame.clone());
                        // Sort to maintain order
                        self.frames
                            .make_contiguous()
                            .sort_by_key(|f| f.frame_number);
                    }
                }
                frames
            }
            None => {
                // Frame is buffered, not ready yet
                // Update max buffered stat
                let current_buffered = self.reorder_buffer.len();
                if current_buffered > self.max_buffered_frames {
                    self.max_buffered_frames = current_buffered;
                }
                Vec::new()
            }
        }
    }

    /// Get reorder buffer statistics
    ///
    /// # Returns
    ///
    /// Tuple of (current_buffered_count, total_delivered_count, max_buffered_count)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let manager = FrameDataManager::new(10, 2);
    /// let (buffered, delivered, max_buffered) = manager.reorder_buffer_stats();
    /// println!("Buffered: {}, Delivered: {}, Max: {}", buffered, delivered, max_buffered);
    /// ```
    pub fn reorder_buffer_stats(&self) -> (usize, u64, usize) {
        let (buffered, delivered) = self.reorder_buffer.stats();
        (buffered, delivered, self.max_buffered_frames)
    }

    /// Reset the frame manager state
    ///
    /// Clears all buffers and resets to initial state.
    /// Useful when starting a new game or recovering from errors.
    pub fn reset(&mut self) {
        self.frames.clear();
        self.frame_history.clear();
        self.current_frame = 0;
        self.frame_counter = 0;
        self.runahead_history.clear();
        self.last_desync_frame = None;
        self.duplicate_detector.clear();
        self.duplicates_detected = 0;
        self.reorder_buffer.clear();
        self.max_buffered_frames = 0;
        self.resend_expiry.clear();
    }

    /// Get statistics about the frame manager state
    pub fn get_stats(&self) -> FrameManagerStats {
        let resend_stats = self.resend_expiry.stats();

        FrameManagerStats {
            current_frame: self.current_frame,
            buffered_frames: self.frames.len(),
            current_runahead: self.get_runahead(),
            min_runahead: self.min_runahead,
            max_ahead: self.max_ahead,
            history_size: self.frame_history.len(),
            last_desync_frame: self.last_desync_frame,
            average_runahead: if self.runahead_history.is_empty() {
                0.0
            } else {
                self.runahead_history.iter().sum::<u32>() as f32
                    / self.runahead_history.len() as f32
            },
            duplicates_detected: self.duplicates_detected,
            reorder_buffered_frames: self.reorder_buffer.len(),
            max_reorder_buffered_frames: self.max_buffered_frames,
            resend_request_expired_count: resend_stats.total_expired,
            resend_request_pending_count: resend_stats.currently_pending,
        }
    }

    /// Handle a frame resend request by retrieving frame data from history
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame number to resend
    ///
    /// # Returns
    ///
    /// Frame data if available in history, None otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// if let Some(frame_data) = manager.handle_resend_request(95) {
    ///     // Resend frame data to requesting player
    /// }
    /// ```
    pub fn handle_resend_request(&self, frame_number: u32) -> Option<FrameData> {
        // First check active frames buffer
        if let Some(frame) = self.frames.iter().find(|f| f.frame_number == frame_number) {
            return Some(frame.clone());
        }

        // Then check history
        self.frame_history
            .iter()
            .find(|f| f.frame_number == frame_number)
            .cloned()
    }

    /// Handle a frame resend request by adding it to the expiry manager
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame number to request resend for
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// manager.request_frame_resend(95);
    /// ```
    pub fn request_frame_resend(&mut self, frame_number: u32) {
        self.resend_expiry.add_request(frame_number);
    }

    /// Get the next resend request to process
    ///
    /// # Returns
    ///
    /// Frame number of the next request to retry, or None if no requests pending
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// if let Some(frame) = manager.get_next_resend_request() {
    ///     // Process resend for frame
    /// }
    /// ```
    pub fn get_next_resend_request(&mut self) -> Option<u32> {
        if let Some(frame_number) = self.resend_expiry.next_to_retry() {
            // Mark as retried (increments retry count)
            self.resend_expiry.mark_retried(frame_number);
            Some(frame_number)
        } else {
            None
        }
    }

    /// Extend TTL for a specific resend request (for slow networks)
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame to extend TTL for
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// manager.extend_resend_ttl(95);
    /// ```
    pub fn extend_resend_ttl(&mut self, frame_number: u32) {
        self.resend_expiry.extend_ttl(frame_number);
    }

    /// Manually trigger cleanup of expired resend requests
    ///
    /// # Returns
    ///
    /// Number of requests removed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// let removed = manager.cleanup_expired_resend_requests();
    /// println!("Removed {} expired requests", removed);
    /// ```
    pub fn cleanup_expired_resend_requests(&mut self) -> usize {
        self.resend_expiry.cleanup_expired()
    }

    /// Get all frames from a specific frame number onwards for resynchronization
    ///
    /// # Arguments
    ///
    /// * `from_frame` - Starting frame number
    ///
    /// # Returns
    ///
    /// Vector of frame data from the requested frame onwards
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::FrameDataManager;
    /// # let mut manager = FrameDataManager::new(10, 2);
    /// let frames = manager.resync_frames(90);
    /// // Send frames 90 onwards to resynchronize
    /// ```
    pub fn resync_frames(&self, from_frame: u32) -> Vec<FrameData> {
        let mut frames = Vec::new();

        // Collect from history
        for frame in &self.frame_history {
            if frame.frame_number >= from_frame {
                frames.push(frame.clone());
            }
        }

        // Collect from active buffer
        for frame in &self.frames {
            if frame.frame_number >= from_frame {
                frames.push(frame.clone());
            }
        }

        // Sort by frame number
        frames.sort_by_key(|f| f.frame_number);
        frames
    }
}

/// Statistics about frame manager state
#[derive(Debug, Clone)]
pub struct FrameManagerStats {
    /// Current execution frame
    pub current_frame: u32,

    /// Number of frames currently buffered
    pub buffered_frames: usize,

    /// Current runahead value
    pub current_runahead: u32,

    /// Minimum runahead required
    pub min_runahead: u32,

    /// Maximum frames ahead allowed
    pub max_ahead: u32,

    /// Number of frames in history
    pub history_size: usize,

    /// Last frame where desync occurred
    pub last_desync_frame: Option<u32>,

    /// Average runahead over recent frames
    pub average_runahead: f32,

    /// Number of duplicate frames detected
    pub duplicates_detected: usize,

    /// Number of frames currently in reorder buffer
    pub reorder_buffered_frames: usize,

    /// Maximum number of frames buffered in reorder buffer
    pub max_reorder_buffered_frames: usize,

    /// Number of expired resend requests
    pub resend_request_expired_count: u64,

    /// Number of pending resend requests
    pub resend_request_pending_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandPayload, NetCommandType};

    fn create_test_command(player_id: u8, frame: u32, seq: u16) -> NetCommand {
        NetCommand::new(
            NetCommandType::KeepAlive,
            player_id,
            frame,
            CommandPayload::KeepAlive,
        )
        .with_sequence(seq)
    }

    #[test]
    fn test_frame_manager_creation() {
        let manager = FrameDataManager::new(10, 2);
        assert_eq!(manager.get_current_frame(), 0);
        assert_eq!(manager.max_ahead, 10);
        assert_eq!(manager.min_runahead, 2);
    }

    #[test]
    fn test_add_command_to_frame() {
        let mut manager = FrameDataManager::new(10, 2);
        let command = create_test_command(0, 5, 1);

        assert!(manager.add_command(5, command).is_ok());

        let frame = manager.get_frame(5).unwrap();
        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.frame_number, 5);
    }

    #[test]
    fn test_cannot_add_to_past_frame() {
        let mut manager = FrameDataManager::new(10, 2);
        manager.current_frame = 10;

        let command = create_test_command(0, 5, 1);
        let result = manager.add_command(5, command);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("past frame"));
    }

    #[test]
    fn test_runahead_violation() {
        let mut manager = FrameDataManager::new(10, 2);
        let command = create_test_command(0, 50, 1);

        let result = manager.add_command(50, command);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds max runahead"));
    }

    #[test]
    fn test_advance_frame() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands to create sufficient runahead
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        assert_eq!(manager.get_current_frame(), 0);
        assert!(manager.advance_frame().is_ok());
        assert_eq!(manager.get_current_frame(), 1);
    }

    #[test]
    fn test_get_runahead() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands to frames 0, 1, 2, 3, 4
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        // Current frame is 0, max frame is 4, so runahead is 4
        assert_eq!(manager.get_runahead(), 4);
    }

    #[test]
    fn test_validate_runahead() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add minimum required frames
        for i in 0..3 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        assert!(manager.is_valid_runahead());
    }

    #[test]
    fn test_crc_validation() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands to frame 5
        let cmd1 = create_test_command(0, 5, 1);
        let cmd2 = create_test_command(1, 5, 2);

        manager.add_command(5, cmd1).unwrap();
        manager.add_command(5, cmd2).unwrap();

        // Mark complete to calculate CRC
        if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == 5) {
            frame.mark_complete();
            let expected_crc = frame.crc;

            // Validate CRC
            assert!(manager.validate_frame_crc(5, expected_crc));

            // Wrong CRC should fail
            assert!(!manager.validate_frame_crc(5, expected_crc + 1));
            assert_eq!(manager.last_desync_frame(), Some(5));
        } else {
            panic!("Frame 5 should exist");
        }
    }

    #[test]
    fn test_frame_history() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add and execute several frames
        for i in 0..10 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();

            if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == i) {
                frame.mark_complete();
            }

            if i >= 2 {
                // Maintain minimum runahead
                manager.advance_frame().unwrap();
            }
        }

        // Should have history
        let history = manager.get_frame_history(5);
        assert!(history.len() > 0);
        assert!(history.len() <= 5);
    }

    #[test]
    fn test_cleanup_old_frames() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add many frames
        for i in 0..50 {
            let cmd = create_test_command(0, i, 1);
            if manager.add_command(i, cmd.clone()).is_ok() {
                if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == i) {
                    frame.mark_complete();
                }
            }

            if i >= 2 && manager.is_valid_runahead() {
                let _ = manager.advance_frame();
            }
        }

        manager.cleanup_old_frames();

        // History should be limited
        assert!(manager.frame_history.len() <= FRAMES_TO_KEEP as usize);

        // Active buffer should not contain very old frames
        let oldest_frame = manager.current_frame.saturating_sub(manager.max_ahead);
        for frame in &manager.frames {
            assert!(frame.frame_number >= oldest_frame);
        }
    }

    #[test]
    fn test_can_execute_frame() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add command to current frame
        let cmd = create_test_command(0, 0, 1);
        manager.add_command(0, cmd).unwrap();

        // Not executable yet (not complete)
        assert!(!manager.can_execute_frame(0));

        // Mark complete
        if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == 0) {
            frame.mark_complete();
        }

        // Now executable
        assert!(manager.can_execute_frame(0));

        // Future frames not executable
        assert!(!manager.can_execute_frame(1));
    }

    #[test]
    fn test_reset() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add some data
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        manager.reset();

        assert_eq!(manager.get_current_frame(), 0);
        assert_eq!(manager.frames.len(), 0);
        assert_eq!(manager.frame_history.len(), 0);
        assert!(manager.last_desync_frame.is_none());
    }

    #[test]
    fn test_stats() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        let stats = manager.get_stats();
        assert_eq!(stats.current_frame, 0);
        assert_eq!(stats.buffered_frames, 5);
        assert_eq!(stats.max_ahead, 10);
        assert_eq!(stats.min_runahead, 2);
    }

    #[test]
    fn test_deterministic_command_ordering() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands in random order
        let cmd3 = create_test_command(1, 5, 3);
        let cmd1 = create_test_command(0, 5, 1);
        let cmd2 = create_test_command(0, 5, 2);
        let cmd4 = create_test_command(1, 5, 1);

        manager.add_command(5, cmd3).unwrap();
        manager.add_command(5, cmd1).unwrap();
        manager.add_command(5, cmd2).unwrap();
        manager.add_command(5, cmd4).unwrap();

        let frame = manager.get_frame(5).unwrap();

        // Commands should be sorted by player_id then sequence
        assert_eq!(frame.commands[0].player_id, 0);
        assert_eq!(frame.commands[0].sequence, 1);
        assert_eq!(frame.commands[1].player_id, 0);
        assert_eq!(frame.commands[1].sequence, 2);
        assert_eq!(frame.commands[2].player_id, 1);
        assert_eq!(frame.commands[2].sequence, 1);
        assert_eq!(frame.commands[3].player_id, 1);
        assert_eq!(frame.commands[3].sequence, 3);
    }

    #[test]
    fn test_buffer_overflow_prevention() {
        let mut manager = FrameDataManager::new(10, 2);

        // Try to add command way beyond max_ahead
        let cmd = create_test_command(0, 100, 1);
        let result = manager.add_command(100, cmd);

        assert!(result.is_err());
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_FRAMES_AHEAD, 128);
        assert_eq!(MIN_RUNAHEAD, 10);
        assert_eq!(FRAME_DATA_LENGTH, 258);
        assert_eq!(FRAMES_TO_KEEP, 65);
    }

    #[test]
    fn test_handle_resend_request_from_active_buffer() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add commands to several frames
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        // Request a frame that's in the active buffer
        let frame_data = manager.handle_resend_request(3);
        assert!(frame_data.is_some());

        let frame = frame_data.unwrap();
        assert_eq!(frame.frame_number, 3);
        assert_eq!(frame.commands.len(), 1);
    }

    #[test]
    fn test_handle_resend_request_from_history() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add and execute several frames to move them to history
        for i in 0..10 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();

            if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == i) {
                frame.mark_complete();
            }

            if i >= 2 && manager.is_valid_runahead() {
                let _ = manager.advance_frame();
            }
        }

        // Request a frame that should be in history
        let frame_data = manager.handle_resend_request(0);
        assert!(frame_data.is_some());

        let frame = frame_data.unwrap();
        assert_eq!(frame.frame_number, 0);
    }

    #[test]
    fn test_handle_resend_request_not_found() {
        let manager = FrameDataManager::new(10, 2);

        // Request a frame that doesn't exist
        let frame_data = manager.handle_resend_request(999);
        assert!(frame_data.is_none());
    }

    #[test]
    fn test_resync_frames() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add and execute several frames
        for i in 0..15 {
            let cmd = create_test_command(0, i, 1);
            if manager.add_command(i, cmd.clone()).is_ok() {
                if let Some(frame) = manager.frames.iter_mut().find(|f| f.frame_number == i) {
                    frame.mark_complete();
                }
            }

            if i >= 2 && manager.is_valid_runahead() {
                let _ = manager.advance_frame();
            }
        }

        // Request resync from frame 5
        let frames = manager.resync_frames(5);

        // Should have frames from 5 onwards
        assert!(!frames.is_empty());
        assert!(frames.iter().all(|f| f.frame_number >= 5));

        // Frames should be sorted
        for i in 1..frames.len() {
            assert!(frames[i].frame_number >= frames[i - 1].frame_number);
        }
    }

    #[test]
    fn test_resync_frames_empty() {
        let manager = FrameDataManager::new(10, 2);

        // Request resync when no frames exist
        let frames = manager.resync_frames(0);
        assert_eq!(frames.len(), 0);
    }

    #[test]
    fn test_resync_frames_future() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add some frames
        for i in 0..5 {
            let cmd = create_test_command(0, i, 1);
            manager.add_command(i, cmd).unwrap();
        }

        // Request resync from future frame
        let frames = manager.resync_frames(100);
        assert_eq!(frames.len(), 0);
    }

    #[test]
    fn test_duplicate_frame_detection() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add command to frame 1
        let cmd1 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd1).unwrap();

        // Mark frame 1 as complete (this calculates CRC and registers with duplicate detector)
        assert!(manager.mark_frame_complete(1));

        // Get the CRC that was calculated
        let frame_crc = manager.get_frame(1).unwrap().crc;

        // Try to add the same frame again with same CRC (should be rejected as duplicate)
        let cmd2 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd2).unwrap(); // Returns Ok but doesn't add

        // Verify duplicate was detected
        assert_eq!(manager.duplicates_detected(), 1);

        // Verify stats include duplicate count
        let stats = manager.get_stats();
        assert_eq!(stats.duplicates_detected, 1);
    }

    #[test]
    fn test_duplicate_different_crc() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add command to frame 1 and mark complete
        let cmd1 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd1).unwrap();
        manager.mark_frame_complete(1);

        // Try to add frame 1 again with different commands (different CRC)
        // This should still be detected as duplicate because we check frame number
        let cmd2 = create_test_command(1, 1, 2); // Different player
        manager.add_command(1, cmd2).unwrap();

        // Should be detected as duplicate (same frame number)
        assert_eq!(manager.duplicates_detected(), 1);
    }

    #[test]
    fn test_different_frame_not_duplicate() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add and complete frame 1
        let cmd1 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd1).unwrap();
        manager.mark_frame_complete(1);

        // Add frame 2 (different frame number)
        let cmd2 = create_test_command(0, 2, 1);
        manager.add_command(2, cmd2).unwrap();

        // Should NOT be duplicate (different frame number)
        assert_eq!(manager.duplicates_detected(), 0);

        // Both frames should exist
        assert!(manager.get_frame(1).is_some());
        assert!(manager.get_frame(2).is_some());
    }

    #[test]
    fn test_duplicate_detector_reset() {
        let mut manager = FrameDataManager::new(10, 2);

        // Add and complete a frame
        let cmd1 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd1).unwrap();
        manager.mark_frame_complete(1);

        // Create a duplicate
        let cmd2 = create_test_command(0, 1, 1);
        manager.add_command(1, cmd2).unwrap();
        assert_eq!(manager.duplicates_detected(), 1);

        // Reset should clear duplicate counter
        manager.reset();
        assert_eq!(manager.duplicates_detected(), 0);
    }

    // Integration tests for reorder buffer
    #[test]
    fn test_process_frame_in_order() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frames 0, 1, 2 in order
        let mut frame0 = FrameData::new(0);
        frame0.add_command(create_test_command(0, 0, 1));
        let result = manager.process_frame(frame0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frame_number, 0);

        let mut frame1 = FrameData::new(1);
        frame1.add_command(create_test_command(0, 1, 1));
        let result = manager.process_frame(frame1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frame_number, 1);

        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        let result = manager.process_frame(frame2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frame_number, 2);

        // Reorder buffer should be empty
        let (buffered, delivered, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 3);
    }

    #[test]
    fn test_process_frame_out_of_order() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 2 first (out of order)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        let result = manager.process_frame(frame2);
        assert_eq!(result.len(), 0); // Buffered

        let (buffered, delivered, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 1);
        assert_eq!(delivered, 0);

        // Process frame 0
        let mut frame0 = FrameData::new(0);
        frame0.add_command(create_test_command(0, 0, 1));
        let result = manager.process_frame(frame0);
        assert_eq!(result.len(), 1); // Only frame 0, waiting for 1
        assert_eq!(result[0].frame_number, 0);

        // Process frame 1 - should deliver 1 and 2
        let mut frame1 = FrameData::new(1);
        frame1.add_command(create_test_command(0, 1, 1));
        let result = manager.process_frame(frame1);
        assert_eq!(result.len(), 2); // Frames 1 and 2
        assert_eq!(result[0].frame_number, 1);
        assert_eq!(result[1].frame_number, 2);

        let (buffered, delivered, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 3);
    }

    #[test]
    fn test_process_frame_partial_out_of_order() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 0
        let mut frame0 = FrameData::new(0);
        frame0.add_command(create_test_command(0, 0, 1));
        let result = manager.process_frame(frame0);
        assert_eq!(result.len(), 1);

        // Process frame 2 (skip 1)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        let result = manager.process_frame(frame2);
        assert_eq!(result.len(), 0); // Buffered

        // Process frame 1 - should deliver 1 and 2
        let mut frame1 = FrameData::new(1);
        frame1.add_command(create_test_command(0, 1, 1));
        let result = manager.process_frame(frame1);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].frame_number, 1);
        assert_eq!(result[1].frame_number, 2);
    }

    #[test]
    fn test_process_frame_buffer_overflow() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 0
        let mut frame0 = FrameData::new(0);
        frame0.add_command(create_test_command(0, 0, 1));
        manager.process_frame(frame0);

        // Process frame 2 (buffer it)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        manager.process_frame(frame2);

        // Fill buffer with frames 3-51
        for i in 3..52 {
            let mut frame = FrameData::new(i);
            frame.add_command(create_test_command(0, i, 1));
            manager.process_frame(frame);
        }

        // Try to add very far ahead frames (should be dropped)
        for i in 100..110 {
            let mut frame = FrameData::new(i);
            frame.add_command(create_test_command(0, i, 1));
            let result = manager.process_frame(frame);
            // Should return empty vec (dropped)
            assert_eq!(result.len(), 0);
        }

        let (buffered, _, max_buffered) = manager.reorder_buffer_stats();
        assert!(buffered <= 50); // Should respect capacity
        assert!(max_buffered <= 50);
    }

    #[test]
    fn test_process_frame_duplicate() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 2
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        let result = manager.process_frame(frame2);
        assert_eq!(result.len(), 0); // Buffered

        // Process frame 2 again (duplicate)
        let mut frame2_dup = FrameData::new(2);
        frame2_dup.add_command(create_test_command(0, 2, 1));
        let result = manager.process_frame(frame2_dup);
        assert_eq!(result.len(), 0); // Empty, ignored

        let (buffered, _, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 1); // Still only 1 buffered
    }

    #[test]
    fn test_reorder_buffer_stats() {
        let mut manager = FrameDataManager::new(10, 2);

        let (buffered, delivered, max_buffered) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 0);
        assert_eq!(max_buffered, 0);

        // Process frame 0
        let mut frame0 = FrameData::new(0);
        frame0.add_command(create_test_command(0, 0, 1));
        manager.process_frame(frame0);

        let (buffered, delivered, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 1);

        // Process frame 2 (buffer it)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        manager.process_frame(frame2);

        let (buffered, delivered, max_buffered) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 1);
        assert_eq!(delivered, 1);
        assert_eq!(max_buffered, 1);
    }

    #[test]
    fn test_manager_stats_includes_reorder_buffer() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 2 (buffer it)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        manager.process_frame(frame2);

        let stats = manager.get_stats();
        assert_eq!(stats.reorder_buffered_frames, 1);
        assert_eq!(stats.max_reorder_buffered_frames, 1);
    }

    #[test]
    fn test_reset_clears_reorder_buffer() {
        let mut manager = FrameDataManager::new(10, 2);

        // Process frame 2 (buffer it)
        let mut frame2 = FrameData::new(2);
        frame2.add_command(create_test_command(0, 2, 1));
        manager.process_frame(frame2);

        let (buffered, _, _) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 1);

        manager.reset();

        let (buffered, delivered, max_buffered) = manager.reorder_buffer_stats();
        assert_eq!(buffered, 0);
        assert_eq!(delivered, 0);
        assert_eq!(max_buffered, 0);
    }
}
