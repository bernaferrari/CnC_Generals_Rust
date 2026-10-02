//! Bandwidth throttling and rate limiting for file transfers
//!
//! Provides configurable bandwidth limits to prevent file transfers from
//! saturating the network connection and impacting game traffic.

use crate::time::NetworkInstant;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Mutable token-bucket state of a throttle.
#[derive(Debug)]
struct ThrottleState {
    /// Maximum bytes per second
    max_bytes_per_second: u64,
    /// Current token count (bytes available)
    tokens: f64,
    /// Last refill time
    last_refill: NetworkInstant,
    /// Bucket capacity (max burst size)
    bucket_capacity: f64,
}

/// Bandwidth throttler using token bucket algorithm
pub struct BandwidthThrottle {
    /// THREAD: the token bucket is the only shared mutable state; the throttle is
    /// handed out as `Arc<BandwidthThrottle>` (see `BandwidthManager`) so concurrent
    /// transfer tasks call `acquire`/`try_acquire` on the same instance. Critical
    /// sections are synchronous and never held across an `.await`.
    state: Mutex<ThrottleState>,
}

impl BandwidthThrottle {
    /// Create a new bandwidth throttle
    ///
    /// # Arguments
    /// * `max_bytes_per_second` - Maximum transfer rate in bytes per second
    /// * `burst_multiplier` - Multiplier for burst capacity (default 2.0)
    pub fn new(max_bytes_per_second: u64, burst_multiplier: f64) -> Self {
        let capacity = max_bytes_per_second as f64 * burst_multiplier;
        Self {
            state: Mutex::new(ThrottleState {
                max_bytes_per_second,
                tokens: capacity,
                last_refill: NetworkInstant::now(),
                bucket_capacity: capacity,
            }),
        }
    }

    /// Create an unlimited throttle (no limits)
    pub fn unlimited() -> Self {
        Self::new(u64::MAX, 1.0)
    }

    /// Update the bandwidth limit
    pub fn set_limit(&self, max_bytes_per_second: u64) {
        let mut state = self.state.lock();
        state.max_bytes_per_second = max_bytes_per_second;

        // Preserve the original capacity propagation: the multiplier is derived
        // from the (already updated) limit so the burst window stays stable.
        let current_limit = max_bytes_per_second as f64;
        let burst_multiplier = if current_limit > 0.0 {
            state.bucket_capacity / current_limit
        } else {
            2.0
        };

        state.bucket_capacity = max_bytes_per_second as f64 * burst_multiplier;
    }

    /// Get current bandwidth limit
    pub fn limit(&self) -> u64 {
        self.state.lock().max_bytes_per_second
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&self) {
        let mut state = self.state.lock();
        let now = NetworkInstant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();

        if elapsed > 0.0 {
            let new_tokens = elapsed * state.max_bytes_per_second as f64;
            state.tokens = (state.tokens + new_tokens).min(state.bucket_capacity);
            state.last_refill = now;
        }
    }

    /// Acquire tokens for a transfer
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to transfer
    ///
    /// Returns when sufficient tokens are available
    pub async fn acquire(&self, bytes: usize) -> Duration {
        let start = NetworkInstant::now();
        let bytes = bytes as f64;

        loop {
            self.refill_tokens();

            let wait_time = {
                let mut state = self.state.lock();
                if state.tokens >= bytes {
                    state.tokens -= bytes;
                    return start.elapsed();
                }

                // Calculate wait time for tokens to refill
                let rate = state.max_bytes_per_second as f64;
                let needed = bytes - state.tokens;
                if rate > 0.0 {
                    Duration::from_secs_f64(needed / rate)
                } else {
                    Duration::from_millis(1)
                }
            };

            // Sleep for a portion of the wait time to allow other operations
            sleep(wait_time.min(Duration::from_millis(10))).await;
        }
    }

    /// Try to acquire tokens without blocking
    ///
    /// Returns true if tokens were acquired, false otherwise
    pub fn try_acquire(&self, bytes: usize) -> bool {
        self.refill_tokens();

        let mut state = self.state.lock();
        let bytes = bytes as f64;

        if state.tokens >= bytes {
            state.tokens -= bytes;
            true
        } else {
            false
        }
    }

    /// Get current available tokens
    pub fn available_tokens(&self) -> u64 {
        self.refill_tokens();
        self.state.lock().tokens as u64
    }

    /// Get statistics about throttle usage
    pub fn stats(&self) -> ThrottleStats {
        self.refill_tokens();
        let state = self.state.lock();
        ThrottleStats {
            max_bytes_per_second: state.max_bytes_per_second,
            available_tokens: state.tokens as u64,
            bucket_capacity: state.bucket_capacity as u64,
        }
    }
}

/// Statistics about bandwidth throttle usage
#[derive(Debug, Clone)]
pub struct ThrottleStats {
    pub max_bytes_per_second: u64,
    pub available_tokens: u64,
    pub bucket_capacity: u64,
}

/// Global bandwidth manager for coordinating multiple transfers
pub struct BandwidthManager {
    /// Upload throttle
    upload_throttle: Arc<BandwidthThrottle>,
    /// Download throttle
    download_throttle: Arc<BandwidthThrottle>,
}

impl BandwidthManager {
    /// Create a new bandwidth manager
    pub fn new(max_upload_bytes_per_second: u64, max_download_bytes_per_second: u64) -> Arc<Self> {
        Arc::new(Self {
            upload_throttle: Arc::new(BandwidthThrottle::new(max_upload_bytes_per_second, 2.0)),
            download_throttle: Arc::new(BandwidthThrottle::new(max_download_bytes_per_second, 2.0)),
        })
    }

    /// Create unlimited bandwidth manager
    pub fn unlimited() -> Arc<Self> {
        Arc::new(Self {
            upload_throttle: Arc::new(BandwidthThrottle::unlimited()),
            download_throttle: Arc::new(BandwidthThrottle::unlimited()),
        })
    }

    /// Get upload throttle
    pub fn upload_throttle(&self) -> Arc<BandwidthThrottle> {
        self.upload_throttle.clone()
    }

    /// Get download throttle
    pub fn download_throttle(&self) -> Arc<BandwidthThrottle> {
        self.download_throttle.clone()
    }

    /// Set upload bandwidth limit
    pub fn set_upload_limit(&self, max_bytes_per_second: u64) {
        self.upload_throttle.set_limit(max_bytes_per_second);
    }

    /// Set download bandwidth limit
    pub fn set_download_limit(&self, max_bytes_per_second: u64) {
        self.download_throttle.set_limit(max_bytes_per_second);
    }

    /// Get bandwidth statistics
    pub fn stats(&self) -> BandwidthStats {
        BandwidthStats {
            upload: self.upload_throttle.stats(),
            download: self.download_throttle.stats(),
        }
    }
}

/// Combined bandwidth statistics
#[derive(Debug, Clone)]
pub struct BandwidthStats {
    pub upload: ThrottleStats,
    pub download: ThrottleStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::NetworkInstant;

    #[tokio::test]
    async fn test_bandwidth_throttle_basic() {
        // 1 MB/s limit
        let throttle = BandwidthThrottle::new(1024 * 1024, 2.0);

        // Should be able to acquire small amount immediately
        let wait = throttle.acquire(1024).await;
        assert!(wait < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_bandwidth_throttle_limit() {
        // 100 KB/s limit (no burst to properly test throttling)
        let throttle = BandwidthThrottle::new(100 * 1024, 1.0);

        let start = NetworkInstant::now();

        // Try to send 200 KB (should take ~2 seconds)
        throttle.acquire(100 * 1024).await;
        throttle.acquire(100 * 1024).await;

        let elapsed = start.elapsed();

        // Should take at least 200ms (very conservative to avoid flakiness on slow CI)
        assert!(
            elapsed >= Duration::from_millis(200),
            "Expected throttling, took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let throttle = BandwidthThrottle::new(1024, 1.0);

        // First acquire should succeed
        assert!(throttle.try_acquire(1024));

        // Immediate second acquire should fail (no tokens)
        assert!(!throttle.try_acquire(1024));

        // Wait for refill (increased wait to handle slow systems)
        sleep(Duration::from_millis(500)).await;

        // Should have some tokens now
        assert!(throttle.try_acquire(100));
    }

    #[tokio::test]
    async fn test_unlimited_throttle() {
        let throttle = BandwidthThrottle::unlimited();

        // Should be able to acquire huge amounts instantly
        let wait = throttle.acquire(1024 * 1024 * 1024).await; // 1 GB
        assert!(wait < Duration::from_millis(1));
    }

    #[tokio::test]
    async fn test_dynamic_limit_change() {
        let throttle = BandwidthThrottle::new(1024, 2.0);

        // Initial limit
        assert_eq!(throttle.limit(), 1024);

        // Change limit
        throttle.set_limit(2048);
        assert_eq!(throttle.limit(), 2048);

        // Should work with new limit
        let wait = throttle.acquire(2048).await;
        assert!(wait < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_bandwidth_manager() {
        let manager = BandwidthManager::new(1024 * 1024, 2048 * 1024);

        // Get throttles
        let upload = manager.upload_throttle();
        let download = manager.download_throttle();

        assert_eq!(upload.limit(), 1024 * 1024);
        assert_eq!(download.limit(), 2048 * 1024);

        // Test setting new limits
        manager.set_upload_limit(512 * 1024);
        manager.set_download_limit(1024 * 1024);

        assert_eq!(upload.limit(), 512 * 1024);
        assert_eq!(download.limit(), 1024 * 1024);
    }

    #[tokio::test]
    async fn test_stats() {
        let throttle = BandwidthThrottle::new(1024 * 1024, 2.0);

        let stats = throttle.stats();
        assert_eq!(stats.max_bytes_per_second, 1024 * 1024);
        assert!(stats.available_tokens > 0);
        assert_eq!(stats.bucket_capacity, 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_burst_capacity() {
        // 1 KB/s with 3x burst
        let throttle = BandwidthThrottle::new(1024, 3.0);

        // Should be able to burst up to 3 KB immediately
        let wait = throttle.acquire(3 * 1024).await;
        assert!(wait < Duration::from_millis(100));

        // Next acquisition should be throttled
        let start = NetworkInstant::now();
        throttle.acquire(1024).await;
        let elapsed = start.elapsed();

        // Should wait for refill
        assert!(elapsed >= Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_concurrent_acquisitions() {
        let throttle = Arc::new(BandwidthThrottle::new(10 * 1024, 2.0));

        let local_set = tokio::task::LocalSet::new();
        local_set
            .run_until(async {
                let mut handles = vec![];

                // Spawn multiple tasks trying to acquire tokens
                for _ in 0..5 {
                    let throttle_clone = Arc::clone(&throttle);
                    handles.push(tokio::task::spawn_local(async move {
                        throttle_clone.acquire(2 * 1024).await;
                    }));
                }

                // All should complete eventually
                for handle in handles {
                    handle.await.unwrap();
                }
            })
            .await;
    }
}
