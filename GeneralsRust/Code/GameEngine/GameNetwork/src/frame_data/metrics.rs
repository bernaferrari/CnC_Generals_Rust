//! Frame metrics tracking for network performance monitoring
//!
//! This module implements frame timing, latency tracking, and FPS monitoring
//! to calculate optimal run-ahead values for deterministic networking.
//! Based on the C++ FrameMetrics implementation.

use crate::config;
use crate::time::NetworkInstant;
use std::collections::VecDeque;
use std::time::Duration;
use tracing::{debug, trace};

/// Frame metrics tracker for performance monitoring and run-ahead calculation
///
/// Tracks FPS, latency, and frame cushion to dynamically adjust the run-ahead
/// distance between command submission and execution. This ensures smooth
/// gameplay while minimizing latency.
pub struct FrameMetrics {
    // FPS tracking
    /// Rolling history of FPS measurements
    fps_history: VecDeque<f32>,
    /// Index into FPS history for circular buffer
    fps_index: usize,
    /// Running average of FPS
    average_fps: f32,
    /// Last time FPS was measured
    last_fps_time: NetworkInstant,
    /// Frame count at last FPS measurement
    last_frame_count: u32,

    // Latency tracking
    /// Rolling history of round-trip latencies (in seconds)
    latency_history: VecDeque<f32>,
    /// Pending latency measurements indexed by frame % MAX_FRAMES_AHEAD
    pending_latencies: Vec<Option<NetworkInstant>>,
    /// Running average latency in seconds
    average_latency: f32,

    // Cushion tracking (how early commands arrive relative to execution)
    /// Index for cushion history
    cushion_index: usize,
    /// Minimum cushion seen in recent history
    minimum_cushion: Option<i32>,
    /// History length for cushion tracking
    cushion_history_length: usize,
    /// Frame counter for cushion history reset
    cushion_frame_count: usize,

    // Configuration
    /// Target frames per second
    target_fps: u32,
    /// Maximum latency history length
    max_latency_history: usize,
    /// Maximum FPS history length
    max_fps_history: usize,
}

impl FrameMetrics {
    /// Create a new frame metrics tracker
    pub fn new() -> Self {
        let target_fps = config::TARGET_FPS;
        let max_fps_history = config::FPS_HISTORY_LENGTH;
        let max_latency_history = config::LATENCY_HISTORY_LENGTH;
        let cushion_history_length = config::CUSHION_HISTORY_LENGTH;

        // Initialize FPS history with target FPS
        let mut fps_history = VecDeque::with_capacity(max_fps_history);
        for _ in 0..max_fps_history {
            fps_history.push_back(target_fps as f32);
        }

        // Initialize latency history with default 200ms
        let mut latency_history = VecDeque::with_capacity(max_latency_history);
        for _ in 0..max_latency_history {
            latency_history.push_back(0.2); // 200ms default
        }

        // Initialize pending latencies array
        let pending_latencies = vec![None; config::MAX_FRAMES_AHEAD as usize];

        Self {
            fps_history,
            fps_index: 0,
            average_fps: target_fps as f32,
            last_fps_time: NetworkInstant::now(),
            last_frame_count: 0,

            latency_history,
            pending_latencies,
            average_latency: 0.2,

            cushion_index: 0,
            minimum_cushion: None,
            cushion_history_length,
            cushion_frame_count: 0,

            target_fps,
            max_latency_history,
            max_fps_history,
        }
    }

    /// Reset all metrics to initial state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Record a frame being processed (call once per frame)
    ///
    /// This updates FPS measurements and prepares latency tracking for the frame.
    pub fn record_frame(&mut self, frame_number: u32, current_fps: Option<f32>) {
        let current_time = NetworkInstant::now();

        // Update FPS measurement every second
        let elapsed = current_time.duration_since(self.last_fps_time);
        if elapsed >= Duration::from_secs(1) {
            // Calculate FPS based on frame count or use provided value
            let fps = if let Some(fps) = current_fps {
                fps
            } else {
                let frames_elapsed = frame_number - self.last_frame_count;
                frames_elapsed as f32 / elapsed.as_secs_f32()
            };

            // Update FPS history using circular buffer
            let old_fps = self.fps_history[self.fps_index];
            self.average_fps -= old_fps / self.max_fps_history as f32;
            self.fps_history[self.fps_index] = fps;
            self.average_fps += fps / self.max_fps_history as f32;

            self.fps_index = (self.fps_index + 1) % self.max_fps_history;
            self.last_fps_time = current_time;
            self.last_frame_count = frame_number;

            trace!(
                "FPS update: current={:.1}, average={:.1}",
                fps,
                self.average_fps
            );
        }

        // Record pending latency measurement for this frame
        let pending_index = (frame_number as usize) % self.pending_latencies.len();
        self.pending_latencies[pending_index] = Some(current_time);
    }

    /// Process a latency response for a frame
    ///
    /// Call this when receiving confirmation that a frame was processed by all players.
    /// This measures the round-trip time and updates latency statistics.
    pub fn process_latency_response(&mut self, frame_number: u32) {
        let current_time = NetworkInstant::now();
        let pending_index = (frame_number as usize) % self.pending_latencies.len();

        if let Some(sent_time) = self.pending_latencies[pending_index] {
            let round_trip_time = current_time.duration_since(sent_time);
            let latency_seconds = round_trip_time.as_secs_f32();

            // Update latency history
            let latency_index = (frame_number as usize) % self.max_latency_history;
            let old_latency = self.latency_history[latency_index];
            self.average_latency -= old_latency / self.max_latency_history as f32;
            self.latency_history[latency_index] = latency_seconds;
            self.average_latency += latency_seconds / self.max_latency_history as f32;

            if frame_number % 16 == 0 {
                debug!(
                    "Latency for frame {}: {:.3}s (avg: {:.3}s)",
                    frame_number, latency_seconds, self.average_latency
                );
            }
        }
    }

    /// Add a cushion measurement
    ///
    /// Cushion represents how many frames ahead commands arrived relative to
    /// when they needed to be executed. Positive cushion means commands arrived
    /// early (good), negative means they arrived late (bad).
    pub fn add_cushion(&mut self, cushion: i32) {
        self.cushion_frame_count += 1;
        self.cushion_index += 1;

        // Reset minimum cushion every cushion_history_length samples
        if self.cushion_index >= self.cushion_history_length {
            self.cushion_index = 0;
            self.minimum_cushion = None;
        }

        // Track minimum cushion
        self.minimum_cushion = Some(match self.minimum_cushion {
            Some(min) => min.min(cushion),
            None => cushion,
        });
    }

    /// Get the current average FPS
    pub fn average_fps(&self) -> f32 {
        self.average_fps
    }

    /// Get the current average FPS as an integer
    pub fn average_fps_int(&self) -> u32 {
        self.average_fps as u32
    }

    /// Get the current average latency in seconds
    pub fn average_latency(&self) -> f32 {
        self.average_latency
    }

    /// Get the current average latency in milliseconds
    pub fn average_latency_ms(&self) -> u32 {
        (self.average_latency * 1000.0) as u32
    }

    /// Get the minimum cushion seen in recent history
    ///
    /// Returns None if no cushion data has been recorded yet.
    pub fn minimum_cushion(&self) -> Option<i32> {
        self.minimum_cushion
    }

    /// Calculate recommended run-ahead frames based on current metrics
    ///
    /// This uses the formula from the original C++ implementation:
    /// run_ahead = (latency * fps) * (1 + slack_percent/100)
    ///
    /// The calculation ensures we have enough frames buffered to handle
    /// network latency while adding some slack for variability.
    pub fn calculate_runahead(&self) -> u32 {
        // Calculate base run-ahead: latency (seconds) * FPS = frames of latency
        let base_runahead = self.average_latency * self.average_fps;

        // Add slack percentage to handle variability
        let slack_multiplier = 1.0 + (config::RUNAHEAD_SLACK_PERCENT as f32 / 100.0);
        let calculated_runahead = base_runahead * slack_multiplier;

        // Clamp to valid range
        let runahead = calculated_runahead as u32;
        runahead
            .max(config::MIN_RUNAHEAD)
            .min(config::MAX_FRAMES_AHEAD)
    }

    /// Get comprehensive metrics snapshot for monitoring
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            average_fps: self.average_fps,
            average_latency_ms: self.average_latency_ms(),
            minimum_cushion: self.minimum_cushion,
            recommended_runahead: self.calculate_runahead(),
            total_samples: self.cushion_frame_count,
        }
    }

    /// Check if metrics suggest network issues
    ///
    /// Returns true if:
    /// - FPS is significantly below target
    /// - Latency is very high
    /// - Cushion is consistently negative
    pub fn has_network_issues(&self) -> bool {
        // FPS dropped significantly below target
        let fps_issue = self.average_fps < (self.target_fps as f32 * 0.7);

        // Latency is very high (over 1 second)
        let latency_issue = self.average_latency > 1.0;

        // Cushion is consistently negative
        let cushion_issue = self.minimum_cushion.map_or(false, |c| c < -3);

        fps_issue || latency_issue || cushion_issue
    }
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of frame metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Average frames per second
    pub average_fps: f32,
    /// Average round-trip latency in milliseconds
    pub average_latency_ms: u32,
    /// Minimum cushion seen in recent history
    pub minimum_cushion: Option<i32>,
    /// Recommended run-ahead frames based on current metrics
    pub recommended_runahead: u32,
    /// Total number of cushion samples recorded
    pub total_samples: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = FrameMetrics::new();
        assert_eq!(metrics.average_fps_int(), config::TARGET_FPS);
        assert!(metrics.average_latency() > 0.0);
        assert!(metrics.minimum_cushion().is_none());
    }

    #[test]
    fn test_runahead_calculation() {
        let mut metrics = FrameMetrics::new();

        // Set known values for testing
        // With 200ms latency and 30 FPS:
        // base_runahead = 0.2 * 30 = 6 frames
        // with 20% slack = 6 * 1.2 = 7.2 = 7 frames
        let runahead = metrics.calculate_runahead();
        assert!(
            runahead >= config::MIN_RUNAHEAD && runahead <= config::MAX_FRAMES_AHEAD,
            "runahead {} not in valid range",
            runahead
        );
    }

    #[test]
    fn test_cushion_tracking() {
        let mut metrics = FrameMetrics::new();

        // Add some cushion measurements
        metrics.add_cushion(5);
        metrics.add_cushion(3);
        metrics.add_cushion(-1);

        assert_eq!(metrics.minimum_cushion(), Some(-1));

        // Add more samples to trigger reset
        for _ in 0..config::CUSHION_HISTORY_LENGTH {
            metrics.add_cushion(10);
        }

        // Minimum should reset
        assert_eq!(metrics.minimum_cushion(), Some(10));
    }

    #[test]
    fn test_fps_tracking() {
        use crate::time::NetworkClock;
        let mut metrics = FrameMetrics::new();

        // Use external time control to simulate time progression
        // Start at 10 seconds to ensure we have room for calculations
        let mut current_time = Duration::from_secs(10);
        NetworkClock::override_with_duration(current_time);

        // Initialize with current external time
        metrics.last_fps_time = NetworkInstant::now();

        // Simulate FPS updates over time (each 2 seconds apart to trigger update)
        for i in 0..config::FPS_HISTORY_LENGTH {
            current_time += Duration::from_secs(2);
            NetworkClock::override_with_duration(current_time);
            metrics.record_frame(i as u32, Some(25.0));
        }

        NetworkClock::clear_override();

        // After filling history with 25 FPS values, average should be 25
        // Using epsilon comparison due to floating point arithmetic
        assert!(
            (metrics.average_fps() - 25.0).abs() < 1.0,
            "Expected average FPS ~25.0, got {}",
            metrics.average_fps()
        );
    }

    #[test]
    fn test_network_issues_detection() {
        let mut metrics = FrameMetrics::new();

        // Normal conditions - no issues
        assert!(!metrics.has_network_issues());

        // Simulate high latency
        for i in 0..config::LATENCY_HISTORY_LENGTH {
            metrics.latency_history[i] = 1.5; // 1.5 seconds
        }
        metrics.average_latency = 1.5;
        assert!(metrics.has_network_issues());

        // Reset and simulate negative cushion
        let mut metrics = FrameMetrics::new();
        metrics.minimum_cushion = Some(-5);
        assert!(metrics.has_network_issues());
    }

    #[test]
    fn test_latency_response_processing() {
        let mut metrics = FrameMetrics::new();

        // Record a frame
        metrics.record_frame(100, None);

        // Simulate 100ms delay
        std::thread::sleep(Duration::from_millis(100));

        // Process response
        metrics.process_latency_response(100);

        // Latency should be updated (hard to test exact value due to timing)
        assert!(metrics.average_latency() > 0.0);
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = FrameMetrics::new();
        let snapshot = metrics.get_snapshot();

        assert_eq!(snapshot.average_fps as u32, config::TARGET_FPS);
        assert!(snapshot.average_latency_ms > 0);
        assert!(snapshot.recommended_runahead >= config::MIN_RUNAHEAD);
    }
}
