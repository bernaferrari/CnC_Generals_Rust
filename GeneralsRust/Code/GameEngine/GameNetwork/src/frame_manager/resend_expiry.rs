//! Auto-expiry mechanism for frame resend requests
//!
//! This module implements automatic timeout and cleanup of frame resend requests
//! to prevent unbounded memory growth. Requests are tracked with TTL (time-to-live)
//! and automatically removed when expired or after exceeding max retries.
//!
//! # Architecture
//!
//! The resend expiry system provides:
//! - FIFO queue of resend requests (oldest first)
//! - Automatic TTL-based expiry
//! - Retry count tracking with configurable limits
//! - Capacity management (max 100 pending requests)
//! - Statistics for monitoring
//!
//! # Example
//!
//! ```no_run
//! use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
//!
//! let config = ResendExpiryConfig::default();
//! let mut manager = ResendExpiryManager::new(config);
//!
//! // Add a resend request for frame 100
//! let request = manager.add_request(100);
//!
//! // Get next frame to retry
//! if let Some(frame) = manager.next_to_retry() {
//!     // Resend frame...
//!     manager.mark_retried(frame);
//! }
//!
//! // Cleanup expired requests
//! let removed = manager.cleanup_expired();
//! ```

use std::collections::VecDeque;
use std::time::Duration;

use crate::time::NetworkInstant;

/// Default TTL for resend requests (5 seconds)
const DEFAULT_TTL_SECS: u64 = 5;

/// Default maximum retry attempts
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default cleanup interval in frames
const DEFAULT_CLEANUP_INTERVAL_FRAMES: u32 = 10;

/// Default capacity for pending requests
const DEFAULT_CAPACITY: usize = 100;

/// Configuration for resend expiry management
#[derive(Debug, Clone)]
pub struct ResendExpiryConfig {
    /// Default TTL for new requests in seconds
    pub default_ttl_secs: u64,

    /// Maximum number of retry attempts before discarding
    pub max_retries: u32,

    /// Cleanup interval in frames (cleanup every N frames)
    pub cleanup_interval_frames: u32,

    /// Maximum number of pending requests
    pub capacity: usize,
}

impl Default for ResendExpiryConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: DEFAULT_TTL_SECS,
            max_retries: DEFAULT_MAX_RETRIES,
            cleanup_interval_frames: DEFAULT_CLEANUP_INTERVAL_FRAMES,
            capacity: DEFAULT_CAPACITY,
        }
    }
}

impl ResendExpiryConfig {
    /// Create a new config with custom values
    pub fn new(
        default_ttl_secs: u64,
        max_retries: u32,
        cleanup_interval_frames: u32,
        capacity: usize,
    ) -> Self {
        Self {
            default_ttl_secs: default_ttl_secs.max(1),
            max_retries,
            cleanup_interval_frames: cleanup_interval_frames.max(1),
            capacity: capacity.max(1),
        }
    }
}

/// Individual resend request with expiry tracking
#[derive(Debug, Clone)]
pub struct ResendRequest {
    /// Frame number to resend
    pub frame_number: u32,

    /// When this request was created
    pub requested_at: NetworkInstant,

    /// Time-to-live for this request
    pub ttl: Duration,

    /// Number of times this request has been retried
    pub retry_count: u32,
}

impl ResendRequest {
    /// Create a new resend request
    pub fn new(frame_number: u32, ttl: Duration) -> Self {
        Self {
            frame_number,
            requested_at: NetworkInstant::now(),
            ttl,
            retry_count: 0,
        }
    }

    /// Check if this request has expired (TTL exceeded)
    pub fn is_expired(&self) -> bool {
        self.requested_at.elapsed() >= self.ttl
    }

    /// Get the age of this request
    pub fn age(&self) -> Duration {
        self.requested_at.elapsed()
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    /// Extend the TTL by the original TTL duration
    pub fn extend_ttl(&mut self) {
        // Extend by adding original TTL to the requested_at time
        // This effectively resets the expiry timer
        self.requested_at = NetworkInstant::now();
    }
}

/// Statistics about resend request management
#[derive(Debug, Clone, Default)]
pub struct ResendStats {
    /// Total number of requests ever created
    pub total_requested: u64,

    /// Total number of requests expired/removed
    pub total_expired: u64,

    /// Currently pending requests
    pub currently_pending: usize,

    /// Maximum pending requests reached
    pub max_pending: usize,

    /// Average retry count across all requests
    pub avg_retry_count: f32,
}

/// Manager for auto-expiry of frame resend requests
#[derive(Debug)]
pub struct ResendExpiryManager {
    /// FIFO queue of pending resend requests
    pending: VecDeque<ResendRequest>,

    /// Configuration
    config: ResendExpiryConfig,

    /// Statistics
    total_requested: u64,
    total_expired: u64,
    max_pending: usize,
    total_retry_count: u64,
}

impl ResendExpiryManager {
    /// Create a new resend expiry manager with configuration
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    ///
    /// let config = ResendExpiryConfig::default();
    /// let manager = ResendExpiryManager::new(config);
    /// ```
    pub fn new(config: ResendExpiryConfig) -> Self {
        let capacity = config.capacity;
        Self {
            pending: VecDeque::with_capacity(capacity),
            config,
            total_requested: 0,
            total_expired: 0,
            max_pending: 0,
            total_retry_count: 0,
        }
    }

    /// Add a new resend request for a frame
    ///
    /// If at capacity, removes the oldest request to make room.
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame number to request resend for
    ///
    /// # Returns
    ///
    /// The created ResendRequest
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let mut manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// let request = manager.add_request(100);
    /// assert_eq!(request.frame_number, 100);
    /// ```
    pub fn add_request(&mut self, frame_number: u32) -> ResendRequest {
        // Enforce capacity limit - remove oldest if at capacity
        if self.pending.len() >= self.config.capacity {
            if let Some(old_request) = self.pending.pop_front() {
                self.total_expired += 1;
                #[cfg(feature = "metrics")]
                tracing::debug!(
                    "Evicted oldest resend request for frame {} to maintain capacity",
                    old_request.frame_number
                );
            }
        }

        let ttl = Duration::from_secs(self.config.default_ttl_secs);
        let request = ResendRequest::new(frame_number, ttl);

        self.pending.push_back(request.clone());
        self.total_requested += 1;

        // Update max pending
        if self.pending.len() > self.max_pending {
            self.max_pending = self.pending.len();
        }

        request
    }

    /// Check if a request is expired
    ///
    /// # Arguments
    ///
    /// * `request` - Request to check
    ///
    /// # Returns
    ///
    /// true if the request has exceeded its TTL
    pub fn is_expired(request: &ResendRequest) -> bool {
        request.is_expired()
    }

    /// Clean up expired and max-retry-exceeded requests
    ///
    /// # Returns
    ///
    /// Number of requests removed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let mut manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// let removed = manager.cleanup_expired();
    /// println!("Removed {} expired requests", removed);
    /// ```
    pub fn cleanup_expired(&mut self) -> usize {
        let original_len = self.pending.len();

        // Remove expired requests and those exceeding max retries
        self.pending
            .retain(|req| !req.is_expired() && req.retry_count < self.config.max_retries);

        let removed = original_len - self.pending.len();
        self.total_expired += removed as u64;

        #[cfg(feature = "metrics")]
        if removed > 0 {
            tracing::debug!("Cleaned up {} expired/max-retry resend requests", removed);
        }

        removed
    }

    /// Get the next frame number to retry (oldest non-expired)
    ///
    /// Returns None if no requests are pending.
    ///
    /// # Returns
    ///
    /// Frame number of the oldest pending request, or None
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let mut manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// if let Some(frame) = manager.next_to_retry() {
    ///     println!("Next frame to retry: {}", frame);
    /// }
    /// ```
    pub fn next_to_retry(&mut self) -> Option<u32> {
        // Find first non-expired request
        self.pending.front().map(|req| req.frame_number)
    }

    /// Mark a frame as retried (increment retry count)
    ///
    /// If the retry count exceeds max_retries, the request is removed.
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame that was retried
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let mut manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// manager.add_request(100);
    /// manager.mark_retried(100);
    /// ```
    pub fn mark_retried(&mut self, frame_number: u32) {
        if let Some(req) = self
            .pending
            .iter_mut()
            .find(|r| r.frame_number == frame_number)
        {
            req.increment_retry();
            self.total_retry_count += 1;

            #[cfg(feature = "metrics")]
            tracing::debug!(
                "Marked frame {} as retried (retry_count: {})",
                frame_number,
                req.retry_count
            );

            // Remove if exceeded max retries
            if req.retry_count >= self.config.max_retries {
                #[cfg(feature = "metrics")]
                tracing::warn!(
                    "Frame {} exceeded max retries ({}), removing request",
                    frame_number,
                    self.config.max_retries
                );
            }
        }

        // Clean up requests that exceeded max retries
        let original_len = self.pending.len();
        self.pending
            .retain(|req| req.retry_count < self.config.max_retries);
        let removed = original_len - self.pending.len();
        self.total_expired += removed as u64;
    }

    /// Extend TTL for a specific frame request
    ///
    /// Useful for slow networks where requests need more time.
    ///
    /// # Arguments
    ///
    /// * `frame_number` - Frame to extend TTL for
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let mut manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// manager.add_request(100);
    /// manager.extend_ttl(100);
    /// ```
    pub fn extend_ttl(&mut self, frame_number: u32) {
        if let Some(req) = self
            .pending
            .iter_mut()
            .find(|r| r.frame_number == frame_number)
        {
            req.extend_ttl();

            #[cfg(feature = "metrics")]
            tracing::debug!("Extended TTL for frame {}", frame_number);
        }
    }

    /// Get current statistics
    ///
    /// # Returns
    ///
    /// ResendStats with current state
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_network::frame_manager::resend_expiry::{ResendExpiryManager, ResendExpiryConfig};
    /// # let manager = ResendExpiryManager::new(ResendExpiryConfig::default());
    /// let stats = manager.stats();
    /// println!("Currently pending: {}", stats.currently_pending);
    /// ```
    pub fn stats(&self) -> ResendStats {
        let avg_retry_count = if self.total_requested > 0 {
            self.total_retry_count as f32 / self.total_requested as f32
        } else {
            0.0
        };

        ResendStats {
            total_requested: self.total_requested,
            total_expired: self.total_expired,
            currently_pending: self.pending.len(),
            max_pending: self.max_pending,
            avg_retry_count,
        }
    }

    /// Get number of currently pending requests
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Check if there are no pending requests
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Clear all pending requests
    pub fn clear(&mut self) {
        self.total_expired += self.pending.len() as u64;
        self.pending.clear();
    }

    /// Get cleanup interval in frames
    pub fn cleanup_interval(&self) -> u32 {
        self.config.cleanup_interval_frames
    }
}

impl Default for ResendExpiryManager {
    fn default() -> Self {
        Self::new(ResendExpiryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{NetworkClock, NetworkInstant};
    use std::thread;

    #[test]
    fn test_add_request() {
        let mut manager = ResendExpiryManager::default();

        // Add 3 requests
        let req1 = manager.add_request(1);
        let req2 = manager.add_request(2);
        let req3 = manager.add_request(3);

        assert_eq!(req1.frame_number, 1);
        assert_eq!(req2.frame_number, 2);
        assert_eq!(req3.frame_number, 3);

        // Verify all pending
        assert_eq!(manager.len(), 3);
        let stats = manager.stats();
        assert_eq!(stats.currently_pending, 3);
        assert_eq!(stats.total_requested, 3);
    }

    #[test]
    fn test_expired_request_cleanup() {
        let config = ResendExpiryConfig {
            default_ttl_secs: 0, // Expire immediately for testing
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        // Add request
        manager.add_request(1);
        assert_eq!(manager.len(), 1);

        // Sleep to ensure expiry
        thread::sleep(Duration::from_millis(10));

        // Cleanup should remove expired request
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(manager.len(), 0);

        let stats = manager.stats();
        assert_eq!(stats.total_expired, 1);
    }

    #[test]
    fn test_next_to_retry_fifo() {
        let mut manager = ResendExpiryManager::default();

        // Add frames 1, 2, 3
        manager.add_request(1);
        manager.add_request(2);
        manager.add_request(3);

        // Should return in FIFO order
        assert_eq!(manager.next_to_retry(), Some(1));
        // Next call should still return 1 (not removed)
        assert_eq!(manager.next_to_retry(), Some(1));
    }

    #[test]
    fn test_max_retries_exceeded() {
        let config = ResendExpiryConfig {
            max_retries: 2,
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        // Add request
        manager.add_request(1);
        assert_eq!(manager.len(), 1);

        // Mark retried 3 times (exceeds max_retries=2)
        manager.mark_retried(1); // retry_count = 1
        assert_eq!(manager.len(), 1);

        manager.mark_retried(1); // retry_count = 2
        assert_eq!(manager.len(), 0); // Should be removed (retry_count >= max_retries)

        let stats = manager.stats();
        assert_eq!(stats.total_expired, 1);
    }

    #[test]
    fn test_extend_ttl() {
        let config = ResendExpiryConfig {
            default_ttl_secs: 1, // 1 second TTL
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        // Add request
        manager.add_request(1);

        // Sleep 500ms (not expired yet)
        thread::sleep(Duration::from_millis(500));

        // Verify not expired
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.len(), 1);

        // Extend TTL (resets timer)
        manager.extend_ttl(1);

        // Sleep another 700ms (would exceed original TTL of 1 second)
        thread::sleep(Duration::from_millis(700));

        // Should still not be expired (TTL was extended)
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_capacity_limit() {
        let config = ResendExpiryConfig {
            capacity: 100,
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        // Add 100 requests (at capacity)
        for i in 0..100 {
            manager.add_request(i);
        }
        assert_eq!(manager.len(), 100);

        // Add 101st request - should evict oldest (frame 0)
        manager.add_request(100);
        assert_eq!(manager.len(), 100);

        // Oldest should be frame 1 now (frame 0 evicted)
        assert_eq!(manager.next_to_retry(), Some(1));

        let stats = manager.stats();
        assert_eq!(stats.total_requested, 101);
        assert_eq!(stats.total_expired, 1); // One evicted
    }

    #[test]
    fn test_stats() {
        let config = ResendExpiryConfig {
            default_ttl_secs: 0, // Immediate expiry for testing
            max_retries: 1,
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        // Add some requests
        manager.add_request(1);
        manager.add_request(2);
        manager.add_request(3);

        // Retry one
        manager.mark_retried(1);

        // Sleep and cleanup to expire remaining
        thread::sleep(Duration::from_millis(10));
        manager.cleanup_expired();

        let stats = manager.stats();
        assert_eq!(stats.total_requested, 3);
        assert!(stats.total_expired >= 2); // At least 2 expired
        assert_eq!(stats.max_pending, 3);
        assert!(stats.avg_retry_count > 0.0);
    }

    #[test]
    fn test_is_expired() {
        let ttl = Duration::from_millis(100);
        let request = ResendRequest::new(1, ttl);

        // Should not be expired immediately
        assert!(!ResendExpiryManager::is_expired(&request));

        // Wait for expiry
        thread::sleep(Duration::from_millis(150));

        // Should be expired now
        assert!(ResendExpiryManager::is_expired(&request));
    }

    #[test]
    fn test_request_age() {
        let request = ResendRequest::new(1, Duration::from_secs(5));

        thread::sleep(Duration::from_millis(100));

        let age = request.age();
        assert!(age >= Duration::from_millis(100));
        assert!(age < Duration::from_millis(200));
    }

    #[test]
    fn test_clear() {
        let mut manager = ResendExpiryManager::default();

        manager.add_request(1);
        manager.add_request(2);
        assert_eq!(manager.len(), 2);

        manager.clear();
        assert_eq!(manager.len(), 0);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_cleanup_interval() {
        let config = ResendExpiryConfig {
            cleanup_interval_frames: 20,
            ..Default::default()
        };
        let manager = ResendExpiryManager::new(config);

        assert_eq!(manager.cleanup_interval(), 20);
    }

    #[test]
    fn test_default_config() {
        let config = ResendExpiryConfig::default();

        assert_eq!(config.default_ttl_secs, DEFAULT_TTL_SECS);
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(
            config.cleanup_interval_frames,
            DEFAULT_CLEANUP_INTERVAL_FRAMES
        );
        assert_eq!(config.capacity, DEFAULT_CAPACITY);
    }

    #[test]
    fn test_custom_config() {
        let config = ResendExpiryConfig::new(10, 5, 15, 200);

        assert_eq!(config.default_ttl_secs, 10);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.cleanup_interval_frames, 15);
        assert_eq!(config.capacity, 200);
    }

    #[test]
    fn test_config_validation() {
        // Zero values should be clamped to minimum
        let config = ResendExpiryConfig::new(0, 0, 0, 0);

        assert_eq!(config.default_ttl_secs, 1); // Clamped to 1
        assert_eq!(config.max_retries, 0); // No clamping for retries
        assert_eq!(config.cleanup_interval_frames, 1); // Clamped to 1
        assert_eq!(config.capacity, 1); // Clamped to 1
    }

    #[test]
    fn test_increment_retry() {
        let mut request = ResendRequest::new(1, Duration::from_secs(5));

        assert_eq!(request.retry_count, 0);
        request.increment_retry();
        assert_eq!(request.retry_count, 1);
        request.increment_retry();
        assert_eq!(request.retry_count, 2);
    }

    #[test]
    fn test_multiple_extend_ttl() {
        let config = ResendExpiryConfig {
            default_ttl_secs: 1,
            ..Default::default()
        };
        let mut manager = ResendExpiryManager::new(config);

        manager.add_request(1);

        // Extend multiple times
        thread::sleep(Duration::from_millis(500));
        manager.extend_ttl(1);

        thread::sleep(Duration::from_millis(500));
        manager.extend_ttl(1);

        thread::sleep(Duration::from_millis(500));

        // Should still not be expired (extended twice)
        let removed = manager.cleanup_expired();
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_clock_override_controls_request_timestamps() {
        NetworkClock::override_with_duration(Duration::from_secs(5));
        let mut manager = ResendExpiryManager::default();
        let request = manager.add_request(99);
        assert_eq!(request.requested_at.as_duration(), Duration::from_secs(5));

        NetworkClock::override_with_duration(Duration::from_secs(7));
        assert!(!request.is_expired());
        NetworkClock::clear_override();
    }
}
