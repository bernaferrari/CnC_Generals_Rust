//! Frame Resend Request Handling for Missing Frame Data Recovery
//!
//! This module implements a robust system for requesting and resending missing frame data
//! to maintain synchronization when network packets are lost. It tracks pending requests,
//! manages resend history, and ensures timely recovery of missing frames.
//!
//! # Architecture
//!
//! The frame resend system operates on a request-response model where:
//! - Players detect missing frames and send resend requests
//! - Requests are tracked and deduplicated to prevent spam
//! - Frame data is packaged and resent to requesting players
//! - History is maintained for debugging and monitoring
//!
//! # Example
//!
//! ```no_run
//! use game_network::frame_resend::{FrameResendManager, FrameResendRequest};
//! use std::time::Duration;
//!
//! let mut manager = FrameResendManager::new();
//! // Request missing frames 100-105
//! let request = manager.request_frames(1, 100, 105)?;
//! // Acknowledge when received
//! manager.acknowledge_request(1, 100)?;
//! # Ok::<(), game_network::error::NetworkError>(())
//! ```

use crate::commands::{CommandPayload, NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Maximum number of pending resend requests per player
const MAX_PENDING_REQUESTS_PER_PLAYER: usize = 10;

/// Maximum frame range for a single resend request
const MAX_FRAME_RANGE: u32 = 50;

/// Maximum age for resend requests before cleanup
/// Currently managed by ResendExpiryManager with TTL-based expiry
#[allow(dead_code)]
const MAX_REQUEST_AGE: Duration = Duration::from_secs(30);

/// Default timeout for resend requests
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Frame resend request tracking individual missing frame ranges
#[derive(Debug, Clone, PartialEq)]
pub struct FrameResendRequest {
    /// Player ID requesting the frames
    pub requesting_player_id: u8,
    /// Starting frame number (inclusive)
    pub start_frame: u32,
    /// Ending frame number (inclusive)
    pub end_frame: u32,
    /// When this request was made (using Instant for timing)
    pub requested_at: NetworkInstant,
    /// Whether this request has been acknowledged
    pub acknowledged: bool,
}

/// Serializable version of FrameResendRequest for network transmission
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializableFrameResendRequest {
    /// Player ID requesting the frames
    pub requesting_player_id: u8,
    /// Starting frame number (inclusive)
    pub start_frame: u32,
    /// Ending frame number (inclusive)
    pub end_frame: u32,
    /// When this request was made (Unix timestamp in seconds)
    pub requested_at_secs: u64,
    /// Whether this request has been acknowledged
    pub acknowledged: bool,
}

impl FrameResendRequest {
    /// Create a new frame resend request
    pub fn new(requesting_player_id: u8, start_frame: u32, end_frame: u32) -> Self {
        Self {
            requesting_player_id,
            start_frame,
            end_frame,
            requested_at: NetworkInstant::now(),
            acknowledged: false,
        }
    }

    /// Convert to serializable format for network transmission
    pub fn to_serializable(&self) -> SerializableFrameResendRequest {
        let requested_at_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        SerializableFrameResendRequest {
            requesting_player_id: self.requesting_player_id,
            start_frame: self.start_frame,
            end_frame: self.end_frame,
            requested_at_secs,
            acknowledged: self.acknowledged,
        }
    }

    /// Create from serializable format
    pub fn from_serializable(s: SerializableFrameResendRequest) -> Self {
        Self {
            requesting_player_id: s.requesting_player_id,
            start_frame: s.start_frame,
            end_frame: s.end_frame,
            requested_at: NetworkInstant::now(), // Use current time since we can't deserialize Instant
            acknowledged: s.acknowledged,
        }
    }

    /// Check if this request has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        self.requested_at.elapsed() >= timeout
    }

    /// Get the number of frames in this request
    pub fn frame_count(&self) -> u32 {
        self.end_frame
            .saturating_sub(self.start_frame)
            .saturating_add(1)
    }

    /// Check if a frame number is within this request range
    pub fn contains_frame(&self, frame: u32) -> bool {
        frame >= self.start_frame && frame <= self.end_frame
    }

    /// Mark this request as acknowledged
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// Get the age of this request
    pub fn age(&self) -> Duration {
        self.requested_at.elapsed()
    }
}

/// Frame resend command format for transmitting frame data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameResendCommand {
    /// Frame number being resent
    pub frame_number: u32,
    /// Number of commands in this frame
    pub command_count: u16,
    /// Commands for this frame
    pub commands: Vec<NetCommand>,
}

impl FrameResendCommand {
    /// Create a new frame resend command
    pub fn new(frame_number: u32, commands: Vec<NetCommand>) -> Self {
        let command_count = commands.len().min(u16::MAX as usize) as u16;
        Self {
            frame_number,
            command_count,
            commands,
        }
    }

    /// Calculate size in bytes
    pub fn size(&self) -> usize {
        std::mem::size_of::<u32>() // frame_number
            + std::mem::size_of::<u16>() // command_count
            + self.commands.iter().map(|cmd| cmd.size()).sum::<usize>()
    }
}

/// Frame Resend Manager handles missing frame data recovery
#[derive(Debug)]
pub struct FrameResendManager {
    /// Pending resend requests per player
    pending_requests: HashMap<u8, Vec<FrameResendRequest>>,
    /// History of all resend requests for debugging
    resend_history: Vec<FrameResendRequest>,
    /// Maximum pending requests per player
    max_pending: usize,
    /// Request timeout duration
    request_timeout: Duration,
}

impl FrameResendManager {
    /// Create a new frame resend manager
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_resend::FrameResendManager;
    ///
    /// let manager = FrameResendManager::new();
    /// ```
    pub fn new() -> Self {
        Self {
            pending_requests: HashMap::new(),
            resend_history: Vec::with_capacity(100),
            max_pending: MAX_PENDING_REQUESTS_PER_PLAYER,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Create a frame resend request
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player requesting the frames
    /// * `start` - Starting frame number (inclusive)
    /// * `end` - Ending frame number (inclusive)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Too many pending requests for this player
    /// - Invalid frame range (start > end)
    /// - Frame range exceeds maximum allowed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let mut manager = FrameResendManager::new();
    /// // Request frames 100-105
    /// let request = manager.request_frames(1, 100, 105)?;
    /// # Ok::<(), game_network::error::NetworkError>(())
    /// ```
    pub fn request_frames(
        &mut self,
        player_id: u8,
        start: u32,
        end: u32,
    ) -> NetworkResult<FrameResendRequest> {
        // Validate frame range
        if start > end {
            return Err(NetworkError::invalid_command(format!(
                "Invalid frame range: start {} > end {}",
                start, end
            )));
        }

        let frame_count = end.saturating_sub(start).saturating_add(1);
        if frame_count > MAX_FRAME_RANGE {
            return Err(NetworkError::invalid_command(format!(
                "Frame range too large: {} frames (max {})",
                frame_count, MAX_FRAME_RANGE
            )));
        }

        // Check pending request limit
        let pending = self.pending_requests.entry(player_id).or_default();
        if pending.len() >= self.max_pending {
            return Err(NetworkError::resource_exhausted(format!(
                "Too many pending resend requests for player {} ({}/{})",
                player_id,
                pending.len(),
                self.max_pending
            )));
        }

        // Check for duplicate or overlapping requests
        for existing in pending.iter() {
            if existing.start_frame <= end && existing.end_frame >= start {
                return Err(NetworkError::invalid_command(format!(
                    "Overlapping frame resend request: existing [{}-{}], new [{}-{}]",
                    existing.start_frame, existing.end_frame, start, end
                )));
            }
        }

        // Create and track the request
        let request = FrameResendRequest::new(player_id, start, end);
        pending.push(request.clone());

        // Add to history
        if self.resend_history.len() >= 1000 {
            self.resend_history.drain(0..100); // Remove oldest 100
        }
        self.resend_history.push(request.clone());

        Ok(request)
    }

    /// Acknowledge receipt of a resend request
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player acknowledging the request
    /// * `start` - Starting frame that was received
    ///
    /// # Errors
    ///
    /// Returns error if no matching pending request is found
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let mut manager = FrameResendManager::new();
    /// manager.acknowledge_request(1, 100)?;
    /// # Ok::<(), game_network::error::NetworkError>(())
    /// ```
    pub fn acknowledge_request(&mut self, player_id: u8, start: u32) -> NetworkResult<()> {
        let pending = self.pending_requests.get_mut(&player_id).ok_or_else(|| {
            NetworkError::frame_sync(format!("No pending requests for player {}", player_id))
        })?;

        // Find and acknowledge the request
        let mut found = false;
        pending.retain_mut(|req| {
            if req.start_frame == start && !req.acknowledged {
                req.acknowledge();
                found = true;
                false // Remove from pending
            } else {
                true // Keep in pending
            }
        });

        if !found {
            return Err(NetworkError::frame_sync(format!(
                "No pending resend request for player {} starting at frame {}",
                player_id, start
            )));
        }

        Ok(())
    }

    /// Get all pending resend requests for a player
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player to check
    ///
    /// # Returns
    ///
    /// Vector of references to pending requests
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let manager = FrameResendManager::new();
    /// let pending = manager.get_pending_requests(1);
    /// println!("Player 1 has {} pending requests", pending.len());
    /// ```
    pub fn get_pending_requests(&self, player_id: u8) -> Vec<&FrameResendRequest> {
        self.pending_requests
            .get(&player_id)
            .map(|reqs| reqs.iter().collect())
            .unwrap_or_default()
    }

    /// Generate resend commands for a frame range
    ///
    /// This method creates NetCommand instances that can be sent to requesting players.
    /// Each frame in the range gets its own resend command.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting frame number (inclusive)
    /// * `end` - Ending frame number (inclusive)
    ///
    /// # Errors
    ///
    /// Returns error if frame range is invalid
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let manager = FrameResendManager::new();
    /// let commands = manager.resend_frames(100, 105)?;
    /// # Ok::<(), game_network::error::NetworkError>(())
    /// ```
    pub fn resend_frames(&self, start: u32, end: u32) -> NetworkResult<Vec<NetCommand>> {
        if start > end {
            return Err(NetworkError::invalid_command(format!(
                "Invalid frame range for resend: start {} > end {}",
                start, end
            )));
        }

        let frame_count = end.saturating_sub(start).saturating_add(1);
        if frame_count > MAX_FRAME_RANGE {
            return Err(NetworkError::invalid_command(format!(
                "Frame range too large for resend: {} frames (max {})",
                frame_count, MAX_FRAME_RANGE
            )));
        }

        // This is a placeholder - in a real implementation, this would:
        // 1. Look up the frame data from frame history
        // 2. Package commands for each frame
        // 3. Create resend commands
        //
        // For now, we create empty resend commands as a template
        let mut commands = Vec::new();
        for frame_num in start..=end {
            let resend_cmd = FrameResendCommand::new(frame_num, Vec::new());

            // Create a NetCommand wrapping the resend data
            let cmd = NetCommand::new(
                NetCommandType::FrameResendRequest,
                0, // Server/host player ID
                frame_num,
                CommandPayload::Generic(bincode::serialize(&resend_cmd).unwrap_or_default()),
            );
            commands.push(cmd);
        }

        Ok(commands)
    }

    /// Get the complete resend history
    ///
    /// # Returns
    ///
    /// Slice of all resend requests in chronological order
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let manager = FrameResendManager::new();
    /// let history = manager.get_resend_history();
    /// println!("Total resend requests: {}", history.len());
    /// ```
    pub fn get_resend_history(&self) -> &[FrameResendRequest] {
        &self.resend_history
    }

    /// Clean up old and timed out requests
    ///
    /// # Arguments
    ///
    /// * `max_age` - Maximum age for requests before removal
    ///
    /// # Returns
    ///
    /// Number of requests cleaned up
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # use std::time::Duration;
    /// # let mut manager = FrameResendManager::new();
    /// let cleaned = manager.cleanup_old_requests(Duration::from_secs(30));
    /// println!("Cleaned up {} old requests", cleaned);
    /// ```
    pub fn cleanup_old_requests(&mut self, max_age: Duration) -> usize {
        let mut cleaned = 0;

        for (_, pending) in self.pending_requests.iter_mut() {
            let original_len = pending.len();
            pending.retain(|req| req.age() < max_age && !req.is_timed_out(self.request_timeout));
            cleaned += original_len - pending.len();
        }

        // Remove empty entries
        self.pending_requests.retain(|_, reqs| !reqs.is_empty());

        cleaned
    }

    /// Check if a resend request is pending for a specific frame
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player to check
    /// * `frame` - Frame number to check
    ///
    /// # Returns
    ///
    /// True if there's a pending request covering this frame
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_resend::FrameResendManager;
    /// # let manager = FrameResendManager::new();
    /// if manager.is_request_pending(1, 100) {
    ///     println!("Player 1 is waiting for frame 100");
    /// }
    /// ```
    pub fn is_request_pending(&self, player_id: u8, frame: u32) -> bool {
        self.pending_requests
            .get(&player_id)
            .map(|reqs| reqs.iter().any(|req| req.contains_frame(frame)))
            .unwrap_or(false)
    }

    /// Get statistics about resend requests
    ///
    /// # Returns
    ///
    /// Statistics tuple: (total_pending, total_history, players_with_pending)
    pub fn get_stats(&self) -> (usize, usize, usize) {
        let total_pending: usize = self.pending_requests.values().map(|v| v.len()).sum();
        let total_history = self.resend_history.len();
        let players_with_pending = self.pending_requests.len();

        (total_pending, total_history, players_with_pending)
    }

    /// Set custom request timeout
    pub fn set_request_timeout(&mut self, timeout: Duration) {
        self.request_timeout = timeout;
    }

    /// Set custom max pending requests per player
    pub fn set_max_pending(&mut self, max_pending: usize) {
        self.max_pending = max_pending.max(1);
    }

    /// Clear all pending requests for a player
    ///
    /// Useful when a player disconnects or game state resets
    pub fn clear_player_requests(&mut self, player_id: u8) -> usize {
        self.pending_requests
            .remove(&player_id)
            .map(|reqs| reqs.len())
            .unwrap_or(0)
    }

    /// Clear all data (reset to initial state)
    pub fn clear_all(&mut self) {
        self.pending_requests.clear();
        self.resend_history.clear();
    }
}

impl Default for FrameResendManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_resend_request_creation() {
        let request = FrameResendRequest::new(1, 100, 105);
        assert_eq!(request.requesting_player_id, 1);
        assert_eq!(request.start_frame, 100);
        assert_eq!(request.end_frame, 105);
        assert!(!request.acknowledged);
        assert_eq!(request.frame_count(), 6);
    }

    #[test]
    fn test_request_contains_frame() {
        let request = FrameResendRequest::new(1, 100, 105);
        assert!(!request.contains_frame(99));
        assert!(request.contains_frame(100));
        assert!(request.contains_frame(103));
        assert!(request.contains_frame(105));
        assert!(!request.contains_frame(106));
    }

    #[test]
    fn test_frame_resend_manager_creation() {
        let manager = FrameResendManager::new();
        let (total_pending, total_history, players_with_pending) = manager.get_stats();
        assert_eq!(total_pending, 0);
        assert_eq!(total_history, 0);
        assert_eq!(players_with_pending, 0);
    }

    #[test]
    fn test_request_frames_success() {
        let mut manager = FrameResendManager::new();
        let result = manager.request_frames(1, 100, 105);
        assert!(result.is_ok());

        let request = result.unwrap();
        assert_eq!(request.requesting_player_id, 1);
        assert_eq!(request.start_frame, 100);
        assert_eq!(request.end_frame, 105);

        let pending = manager.get_pending_requests(1);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn test_request_frames_invalid_range() {
        let mut manager = FrameResendManager::new();

        // Start > end
        let result = manager.request_frames(1, 105, 100);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid frame range"));
    }

    #[test]
    fn test_request_frames_range_too_large() {
        let mut manager = FrameResendManager::new();

        // Range exceeds MAX_FRAME_RANGE
        let result = manager.request_frames(1, 0, MAX_FRAME_RANGE + 1);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Frame range too large"));
    }

    #[test]
    fn test_request_frames_too_many_pending() {
        let mut manager = FrameResendManager::new();
        manager.set_max_pending(2);

        // Add 2 requests (at limit)
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(1, 110, 115).is_ok());

        // Third request should fail
        let result = manager.request_frames(1, 120, 125);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Too many pending resend requests"));
    }

    #[test]
    fn test_request_frames_overlapping() {
        let mut manager = FrameResendManager::new();

        // First request
        assert!(manager.request_frames(1, 100, 110).is_ok());

        // Overlapping request should fail
        let result = manager.request_frames(1, 105, 115);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Overlapping"));
    }

    #[test]
    fn test_acknowledge_request() {
        let mut manager = FrameResendManager::new();

        // Create a request
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert_eq!(manager.get_pending_requests(1).len(), 1);

        // Acknowledge it
        assert!(manager.acknowledge_request(1, 100).is_ok());
        assert_eq!(manager.get_pending_requests(1).len(), 0);
    }

    #[test]
    fn test_acknowledge_request_not_found() {
        let mut manager = FrameResendManager::new();

        // Try to acknowledge non-existent request
        let result = manager.acknowledge_request(1, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_pending_requests() {
        let mut manager = FrameResendManager::new();

        // No requests initially
        assert_eq!(manager.get_pending_requests(1).len(), 0);

        // Add requests
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(1, 110, 115).is_ok());
        assert_eq!(manager.get_pending_requests(1).len(), 2);

        // Different player
        assert!(manager.request_frames(2, 200, 205).is_ok());
        assert_eq!(manager.get_pending_requests(2).len(), 1);
        assert_eq!(manager.get_pending_requests(1).len(), 2);
    }

    #[test]
    fn test_resend_frames() {
        let manager = FrameResendManager::new();

        // Valid range
        let result = manager.resend_frames(100, 105);
        assert!(result.is_ok());
        let commands = result.unwrap();
        assert_eq!(commands.len(), 6);

        // Check frame numbers
        for (i, cmd) in commands.iter().enumerate() {
            assert_eq!(cmd.execution_frame, 100 + i as u32);
            assert_eq!(cmd.command_type, NetCommandType::FrameResendRequest);
        }
    }

    #[test]
    fn test_resend_frames_invalid_range() {
        let manager = FrameResendManager::new();

        // Start > end
        let result = manager.resend_frames(105, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_resend_history() {
        let mut manager = FrameResendManager::new();

        // Initially empty
        assert_eq!(manager.get_resend_history().len(), 0);

        // Add requests
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(2, 200, 205).is_ok());

        // History should contain both
        let history = manager.get_resend_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].requesting_player_id, 1);
        assert_eq!(history[1].requesting_player_id, 2);
    }

    #[test]
    fn test_cleanup_old_requests() {
        let mut manager = FrameResendManager::new();

        // Add a request
        assert!(manager.request_frames(1, 100, 105).is_ok());

        // Should not clean up immediately
        let cleaned = manager.cleanup_old_requests(Duration::from_secs(1));
        assert_eq!(cleaned, 0);

        // Sleep to age the request
        std::thread::sleep(Duration::from_millis(100));

        // Clean with very short max age
        let cleaned = manager.cleanup_old_requests(Duration::from_millis(50));
        assert_eq!(cleaned, 1);
        assert_eq!(manager.get_pending_requests(1).len(), 0);
    }

    #[test]
    fn test_is_request_pending() {
        let mut manager = FrameResendManager::new();

        // No pending request
        assert!(!manager.is_request_pending(1, 100));

        // Add request
        assert!(manager.request_frames(1, 100, 105).is_ok());

        // Check frames in range
        assert!(manager.is_request_pending(1, 100));
        assert!(manager.is_request_pending(1, 103));
        assert!(manager.is_request_pending(1, 105));

        // Check frames outside range
        assert!(!manager.is_request_pending(1, 99));
        assert!(!manager.is_request_pending(1, 106));

        // Check different player
        assert!(!manager.is_request_pending(2, 100));
    }

    #[test]
    fn test_get_stats() {
        let mut manager = FrameResendManager::new();

        // Initially empty
        let (total_pending, total_history, players) = manager.get_stats();
        assert_eq!(total_pending, 0);
        assert_eq!(total_history, 0);
        assert_eq!(players, 0);

        // Add requests
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(1, 110, 115).is_ok());
        assert!(manager.request_frames(2, 200, 205).is_ok());

        let (total_pending, total_history, players) = manager.get_stats();
        assert_eq!(total_pending, 3);
        assert_eq!(total_history, 3);
        assert_eq!(players, 2);

        // Acknowledge one
        assert!(manager.acknowledge_request(1, 100).is_ok());

        let (total_pending, total_history, players) = manager.get_stats();
        assert_eq!(total_pending, 2);
        assert_eq!(total_history, 3); // History doesn't shrink
        assert_eq!(players, 2);
    }

    #[test]
    fn test_clear_player_requests() {
        let mut manager = FrameResendManager::new();

        // Add requests for multiple players
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(1, 110, 115).is_ok());
        assert!(manager.request_frames(2, 200, 205).is_ok());

        // Clear player 1
        let cleared = manager.clear_player_requests(1);
        assert_eq!(cleared, 2);
        assert_eq!(manager.get_pending_requests(1).len(), 0);
        assert_eq!(manager.get_pending_requests(2).len(), 1);
    }

    #[test]
    fn test_clear_all() {
        let mut manager = FrameResendManager::new();

        // Add some requests
        assert!(manager.request_frames(1, 100, 105).is_ok());
        assert!(manager.request_frames(2, 200, 205).is_ok());

        manager.clear_all();

        let (total_pending, total_history, players) = manager.get_stats();
        assert_eq!(total_pending, 0);
        assert_eq!(total_history, 0);
        assert_eq!(players, 0);
    }

    #[test]
    fn test_frame_resend_command() {
        let commands = vec![NetCommand::keep_alive(1), NetCommand::keep_alive(2)];

        let resend_cmd = FrameResendCommand::new(100, commands.clone());
        assert_eq!(resend_cmd.frame_number, 100);
        assert_eq!(resend_cmd.command_count, 2);
        assert_eq!(resend_cmd.commands.len(), 2);
        assert!(resend_cmd.size() > 0);
    }

    #[test]
    fn test_custom_configuration() {
        let mut manager = FrameResendManager::new();

        // Set custom timeout
        manager.set_request_timeout(Duration::from_secs(10));

        // Set custom max pending
        manager.set_max_pending(5);

        // Verify we can add up to 5 requests
        for i in 0..5 {
            let start = i * 10;
            assert!(manager.request_frames(1, start, start + 5).is_ok());
        }

        // 6th should fail
        let result = manager.request_frames(1, 50, 55);
        assert!(result.is_err());
    }

    #[test]
    fn test_request_acknowledgment() {
        let mut request = FrameResendRequest::new(1, 100, 105);
        assert!(!request.acknowledged);

        request.acknowledge();
        assert!(request.acknowledged);
    }
}
