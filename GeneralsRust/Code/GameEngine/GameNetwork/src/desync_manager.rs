//! Desynchronization detection and recovery mechanism
//!
//! This module provides comprehensive desync detection, reporting, and recovery
//! functionality for deterministic lockstep multiplayer gameplay. It tracks CRC
//! mismatches between players, manages recovery mode, and provides metrics for
//! debugging and monitoring game synchronization health.

use crate::commands::{CommandPayload, NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Information about a detected desynchronization event
#[derive(Debug, Clone)]
pub struct DesyncInfo {
    /// Frame number where desync was detected
    pub frame_number: u32,
    /// Expected CRC checksum value
    pub expected_crc: u32,
    /// Received CRC checksum value (mismatch)
    pub received_crc: u32,
    /// Timestamp when desync was detected
    pub detected_at: NetworkInstant,
    /// Player ID who reported the mismatched CRC
    pub player_id: u8,
}

impl DesyncInfo {
    /// Create a new desync info record
    pub fn new(frame_number: u32, expected_crc: u32, received_crc: u32, player_id: u8) -> Self {
        Self {
            frame_number,
            expected_crc,
            received_crc,
            detected_at: NetworkInstant::now(),
            player_id,
        }
    }

    /// Get duration since this desync was detected
    pub fn age(&self) -> Duration {
        self.detected_at.elapsed()
    }

    /// Check if CRC values match
    pub fn is_match(&self) -> bool {
        self.expected_crc == self.received_crc
    }
}

/// Metrics tracking for desync detection system
#[derive(Debug, Clone, Default)]
pub struct DesyncMetrics {
    /// Total number of desyncs detected
    pub total_desyncs: u64,
    /// Desyncs per player
    pub desyncs_per_player: [u64; 8],
    /// Number of recovery attempts
    pub recovery_attempts: u64,
    /// Number of successful recoveries
    pub successful_recoveries: u64,
    /// Total time spent in desync state (milliseconds)
    pub total_desync_time_ms: u64,
    /// Last desync frame number
    pub last_desync_frame: Option<u32>,
}

impl DesyncMetrics {
    /// Calculate recovery success rate as a percentage
    pub fn recovery_success_rate(&self) -> f32 {
        if self.recovery_attempts == 0 {
            return 100.0;
        }
        (self.successful_recoveries as f32 / self.recovery_attempts as f32) * 100.0
    }

    /// Get desyncs for a specific player
    pub fn player_desyncs(&self, player_id: u8) -> u64 {
        if (player_id as usize) < self.desyncs_per_player.len() {
            self.desyncs_per_player[player_id as usize]
        } else {
            0
        }
    }

    /// Increment desync count for a player
    fn increment_player_desyncs(&mut self, player_id: u8) {
        if (player_id as usize) < self.desyncs_per_player.len() {
            self.desyncs_per_player[player_id as usize] += 1;
        }
    }

    /// Record a recovery attempt
    fn record_recovery_attempt(&mut self) {
        self.recovery_attempts += 1;
    }

    /// Record a successful recovery
    fn record_successful_recovery(&mut self) {
        self.successful_recoveries += 1;
    }
}

/// Manager for detecting and recovering from game state desynchronization
pub struct DesyncManager {
    /// List of detected desyncs
    detected_desyncs: Vec<DesyncInfo>,
    /// Maximum number of desyncs allowed before taking action
    max_desyncs_allowed: u32,
    /// Whether the game is currently desynchronized
    is_desynchronized: bool,
    /// Whether recovery mode is active
    desync_recovery_mode: bool,
    /// Last known good frame number (for recovery)
    last_known_good_frame: u32,
    /// Metrics tracking
    metrics: DesyncMetrics,
    /// Timestamp when desync state was entered
    desync_start_time: Option<NetworkInstant>,
}

impl DesyncManager {
    /// Create a new desync manager
    ///
    /// # Arguments
    /// * `max_desyncs` - Maximum number of desyncs to tolerate before flagging as desynchronized
    ///
    /// # Returns
    /// A new `DesyncManager` instance
    pub fn new(max_desyncs: u32) -> Self {
        info!(
            "Initializing DesyncManager with max_desyncs={}",
            max_desyncs
        );

        Self {
            detected_desyncs: Vec::new(),
            max_desyncs_allowed: max_desyncs,
            is_desynchronized: false,
            desync_recovery_mode: false,
            last_known_good_frame: 0,
            metrics: DesyncMetrics::default(),
            desync_start_time: None,
        }
    }

    /// Check a frame's CRC for desynchronization
    ///
    /// Compares the expected CRC with the received CRC and reports a desync
    /// if they don't match.
    ///
    /// # Arguments
    /// * `frame` - Frame number being checked
    /// * `expected` - Expected CRC checksum
    /// * `received` - Received CRC checksum
    /// * `player_id` - Player who provided the CRC
    ///
    /// # Returns
    /// Ok if CRC matches or desync is within tolerance, Err if desynchronization is critical
    pub fn check_frame_crc(
        &mut self,
        frame: u32,
        expected: u32,
        received: u32,
        player_id: u8,
    ) -> NetworkResult<()> {
        if expected == received {
            debug!(
                "CRC check passed for frame {} from player {}: 0x{:08X}",
                frame, player_id, expected
            );
            return Ok(());
        }

        // CRC mismatch detected
        warn!(
            "CRC mismatch detected at frame {} from player {}: expected 0x{:08X}, received 0x{:08X}",
            frame, player_id, expected, received
        );

        self.report_desync(frame, expected, received, player_id);

        // Check if we've exceeded the desync threshold
        if self.detected_desyncs.len() > self.max_desyncs_allowed as usize {
            error!(
                "Desync threshold exceeded: {} desyncs detected (max: {})",
                self.detected_desyncs.len(),
                self.max_desyncs_allowed
            );

            self.is_desynchronized = true;

            if self.desync_start_time.is_none() {
                self.desync_start_time = Some(NetworkInstant::now());
            }

            return Err(NetworkError::FrameSync {
                message: format!(
                    "Game desynchronized at frame {} (player {}): CRC mismatch",
                    frame, player_id
                ),
            });
        }

        Ok(())
    }

    /// Check if the game is currently desynchronized
    pub fn is_desynchronized(&self) -> bool {
        self.is_desynchronized
    }

    /// Report a desynchronization event
    ///
    /// Records the desync information for tracking and debugging purposes.
    ///
    /// # Arguments
    /// * `frame` - Frame number where desync occurred
    /// * `expected` - Expected CRC value
    /// * `received` - Received CRC value
    /// * `player_id` - Player ID who reported the mismatch
    pub fn report_desync(&mut self, frame: u32, expected: u32, received: u32, player_id: u8) {
        let desync_info = DesyncInfo::new(frame, expected, received, player_id);

        info!(
            "Desync reported: frame={}, player={}, expected=0x{:08X}, received=0x{:08X}",
            frame, player_id, expected, received
        );

        self.detected_desyncs.push(desync_info);

        // Update metrics
        self.metrics.total_desyncs += 1;
        self.metrics.increment_player_desyncs(player_id);
        self.metrics.last_desync_frame = Some(frame);
    }

    /// Create a resync request command for the specified frame
    ///
    /// # Arguments
    /// * `frame` - Frame number to resync from
    ///
    /// # Returns
    /// A `NetCommand` requesting frame resync
    pub fn request_resync(&self, frame: u32) -> NetCommand {
        info!("Requesting resync from frame {}", frame);

        NetCommand::new(
            NetCommandType::FrameResendRequest,
            0, // Will be set by sender
            frame,
            CommandPayload::FrameInfo(crate::commands::FrameInfoData {
                frame,
                command_count: 0,
                checksum: 0,
            }),
        )
    }

    /// Enter recovery mode and prepare for state resynchronization
    ///
    /// # Arguments
    /// * `last_good_frame` - Last frame number known to be in sync
    pub fn enter_recovery_mode(&mut self, last_good_frame: u32) {
        if self.desync_recovery_mode {
            warn!("Already in recovery mode, ignoring enter request");
            return;
        }

        info!(
            "Entering desync recovery mode from frame {}",
            last_good_frame
        );

        self.desync_recovery_mode = true;
        self.last_known_good_frame = last_good_frame;
        self.metrics.record_recovery_attempt();

        if self.desync_start_time.is_none() {
            self.desync_start_time = Some(NetworkInstant::now());
        }
    }

    /// Exit recovery mode after successful resynchronization
    pub fn exit_recovery_mode(&mut self) {
        if !self.desync_recovery_mode {
            debug!("Not in recovery mode, ignoring exit request");
            return;
        }

        info!("Exiting desync recovery mode");

        self.desync_recovery_mode = false;
        self.is_desynchronized = false;

        // Record time spent in desync state
        if let Some(start_time) = self.desync_start_time.take() {
            let duration = start_time.elapsed();
            self.metrics.total_desync_time_ms += duration.as_millis() as u64;
        }

        self.metrics.record_successful_recovery();

        // Clear old desyncs on successful recovery
        self.detected_desyncs.clear();
    }

    /// Check if recovery mode is active
    pub fn is_in_recovery_mode(&self) -> bool {
        self.desync_recovery_mode
    }

    /// Get the last known good frame number
    pub fn last_known_good_frame(&self) -> u32 {
        self.last_known_good_frame
    }

    /// Get a reference to all detected desyncs
    pub fn get_desyncs(&self) -> &[DesyncInfo] {
        &self.detected_desyncs
    }

    /// Get a mutable reference to metrics
    pub fn metrics(&self) -> &DesyncMetrics {
        &self.metrics
    }

    /// Clear all recorded desync information
    ///
    /// This should typically only be called after successful recovery
    /// or when starting a new game session.
    pub fn clear_desyncs(&mut self) {
        info!("Clearing all desync records");

        self.detected_desyncs.clear();
        self.is_desynchronized = false;
        self.desync_recovery_mode = false;
        self.desync_start_time = None;
    }

    /// Reset the desync manager to initial state
    ///
    /// Clears all state and metrics. Use when starting a new game.
    pub fn reset(&mut self) {
        info!("Resetting DesyncManager");

        self.detected_desyncs.clear();
        self.is_desynchronized = false;
        self.desync_recovery_mode = false;
        self.last_known_good_frame = 0;
        self.desync_start_time = None;
        // Note: We preserve metrics across resets for debugging purposes
    }

    /// Get the number of detected desyncs
    pub fn desync_count(&self) -> usize {
        self.detected_desyncs.len()
    }

    /// Update the last known good frame
    ///
    /// Should be called periodically during normal gameplay to track
    /// the latest frame that is confirmed to be in sync.
    pub fn update_last_known_good_frame(&mut self, frame: u32) {
        if frame > self.last_known_good_frame {
            self.last_known_good_frame = frame;
            debug!("Updated last known good frame to {}", frame);
        }
    }
}

impl Default for DesyncManager {
    fn default() -> Self {
        Self::new(10) // Default: allow up to 10 desyncs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desync_manager_creation() {
        let manager = DesyncManager::new(5);
        assert_eq!(manager.max_desyncs_allowed, 5);
        assert!(!manager.is_desynchronized());
        assert!(!manager.is_in_recovery_mode());
        assert_eq!(manager.desync_count(), 0);
    }

    #[test]
    fn test_crc_check_matching() {
        let mut manager = DesyncManager::new(5);

        let result = manager.check_frame_crc(100, 0x12345678, 0x12345678, 0);
        assert!(result.is_ok());
        assert_eq!(manager.desync_count(), 0);
        assert!(!manager.is_desynchronized());
    }

    #[test]
    fn test_crc_check_mismatch() {
        let mut manager = DesyncManager::new(5);

        let result = manager.check_frame_crc(100, 0x12345678, 0x87654321, 1);
        assert!(result.is_ok()); // First desync is within tolerance
        assert_eq!(manager.desync_count(), 1);
        assert!(!manager.is_desynchronized()); // Not yet desynchronized
    }

    #[test]
    fn test_desync_threshold_exceeded() {
        let mut manager = DesyncManager::new(2);

        // Report multiple desyncs
        let _ = manager.check_frame_crc(100, 0x1111, 0x2222, 0);
        let _ = manager.check_frame_crc(101, 0x3333, 0x4444, 1);
        let _ = manager.check_frame_crc(102, 0x5555, 0x6666, 2);

        // Third desync should exceed threshold (max=2)
        let result = manager.check_frame_crc(103, 0x7777, 0x8888, 3);
        assert!(result.is_err());
        assert!(manager.is_desynchronized());
    }

    #[test]
    fn test_report_desync() {
        let mut manager = DesyncManager::new(10);

        manager.report_desync(100, 0xAAAA, 0xBBBB, 2);

        assert_eq!(manager.desync_count(), 1);
        assert_eq!(manager.metrics().total_desyncs, 1);
        assert_eq!(manager.metrics().player_desyncs(2), 1);
        assert_eq!(manager.metrics().last_desync_frame, Some(100));
    }

    #[test]
    fn test_recovery_mode() {
        let mut manager = DesyncManager::new(5);

        assert!(!manager.is_in_recovery_mode());

        manager.enter_recovery_mode(95);
        assert!(manager.is_in_recovery_mode());
        assert_eq!(manager.last_known_good_frame(), 95);
        assert_eq!(manager.metrics().recovery_attempts, 1);

        manager.exit_recovery_mode();
        assert!(!manager.is_in_recovery_mode());
        assert_eq!(manager.metrics().successful_recoveries, 1);
        assert_eq!(manager.desync_count(), 0); // Cleared on successful recovery
    }

    #[test]
    fn test_resync_request_creation() {
        let manager = DesyncManager::new(5);

        let command = manager.request_resync(100);
        assert_eq!(command.command_type, NetCommandType::FrameResendRequest);
        assert_eq!(command.execution_frame, 100);
    }

    #[test]
    fn test_clear_desyncs() {
        let mut manager = DesyncManager::new(5);

        manager.report_desync(100, 0x1111, 0x2222, 0);
        manager.report_desync(101, 0x3333, 0x4444, 1);
        manager.enter_recovery_mode(95);

        assert_eq!(manager.desync_count(), 2);
        assert!(manager.is_in_recovery_mode());

        manager.clear_desyncs();

        assert_eq!(manager.desync_count(), 0);
        assert!(!manager.is_in_recovery_mode());
        assert!(!manager.is_desynchronized());
    }

    #[test]
    fn test_metrics_tracking() {
        let mut manager = DesyncManager::new(10);

        // Report desyncs from different players
        manager.report_desync(100, 0x1111, 0x2222, 0);
        manager.report_desync(101, 0x3333, 0x4444, 0);
        manager.report_desync(102, 0x5555, 0x6666, 1);
        manager.report_desync(103, 0x7777, 0x8888, 2);

        let metrics = manager.metrics();
        assert_eq!(metrics.total_desyncs, 4);
        assert_eq!(metrics.player_desyncs(0), 2);
        assert_eq!(metrics.player_desyncs(1), 1);
        assert_eq!(metrics.player_desyncs(2), 1);
        assert_eq!(metrics.last_desync_frame, Some(103));
    }

    #[test]
    fn test_recovery_success_rate() {
        let mut manager = DesyncManager::new(5);

        // No attempts yet
        assert_eq!(manager.metrics().recovery_success_rate(), 100.0);

        // One successful recovery
        manager.enter_recovery_mode(90);
        manager.exit_recovery_mode();
        assert_eq!(manager.metrics().recovery_success_rate(), 100.0);

        // One failed recovery (entered but not exited)
        manager.enter_recovery_mode(95);
        assert_eq!(manager.metrics().recovery_success_rate(), 50.0);
    }

    #[test]
    fn test_update_last_known_good_frame() {
        let mut manager = DesyncManager::new(5);

        assert_eq!(manager.last_known_good_frame(), 0);

        manager.update_last_known_good_frame(100);
        assert_eq!(manager.last_known_good_frame(), 100);

        // Should not go backwards
        manager.update_last_known_good_frame(50);
        assert_eq!(manager.last_known_good_frame(), 100);

        // Should advance forward
        manager.update_last_known_good_frame(150);
        assert_eq!(manager.last_known_good_frame(), 150);
    }

    #[test]
    fn test_desync_info() {
        let info = DesyncInfo::new(100, 0x1234, 0x5678, 2);

        assert_eq!(info.frame_number, 100);
        assert_eq!(info.expected_crc, 0x1234);
        assert_eq!(info.received_crc, 0x5678);
        assert_eq!(info.player_id, 2);
        assert!(!info.is_match());

        let matching = DesyncInfo::new(100, 0x1234, 0x1234, 2);
        assert!(matching.is_match());
    }

    #[test]
    fn test_reset() {
        let mut manager = DesyncManager::new(5);

        manager.report_desync(100, 0x1111, 0x2222, 0);
        manager.enter_recovery_mode(95);
        manager.update_last_known_good_frame(100);

        manager.reset();

        assert_eq!(manager.desync_count(), 0);
        assert!(!manager.is_in_recovery_mode());
        assert!(!manager.is_desynchronized());
        assert_eq!(manager.last_known_good_frame(), 0);
        // Metrics are preserved
        assert!(manager.metrics().total_desyncs > 0);
    }

    #[test]
    fn test_multiple_players_desync_tracking() {
        let mut manager = DesyncManager::new(20);

        // Simulate multiple players reporting desyncs
        for player_id in 0..8 {
            for frame in 0..3 {
                manager.report_desync(100 + frame, 0x1000 + frame, 0x2000 + frame, player_id);
            }
        }

        // Check metrics
        let metrics = manager.metrics();
        assert_eq!(metrics.total_desyncs, 24); // 8 players * 3 frames
        for player_id in 0..8 {
            assert_eq!(metrics.player_desyncs(player_id), 3);
        }
    }

    #[test]
    fn test_desync_age() {
        let info = DesyncInfo::new(100, 0x1234, 0x5678, 0);

        // Age should be very small immediately after creation
        let age = info.age();
        assert!(age.as_millis() < 100);
    }
}
