//! Connection timeout detection and monitoring
//!
//! This module provides timeout detection mechanisms matching the C++ original's
//! retry logic and adds comprehensive connection health monitoring.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::time::Duration;
use tracing::{debug, warn};

/// Configuration for timeout detection
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Connection timeout duration - if no activity for this long, connection is dead
    pub connection_timeout: Duration,

    /// Keep-alive interval - send keep-alive messages at this interval
    pub keepalive_interval: Duration,

    /// How long to wait for an acknowledgment before retry
    pub ack_timeout: Duration,

    /// Maximum number of retries before giving up
    pub max_retries: u32,

    /// Idle threshold - connection is considered idle after this duration
    pub idle_threshold: Duration,

    /// Warning threshold - warn about high latency after this
    pub latency_warning_threshold_ms: f64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(10),
            ack_timeout: Duration::from_millis(2000), // Matches C++ m_retryTime
            max_retries: 5,
            idle_threshold: Duration::from_secs(5),
            latency_warning_threshold_ms: 500.0,
        }
    }
}

/// Connection health monitor
///
/// Tracks connection health metrics and detects various timeout conditions
#[derive(Debug)]
pub struct ConnectionMonitor {
    /// Configuration
    config: TimeoutConfig,

    /// Last time we received any data
    last_receive_time: NetworkInstant,

    /// Last time we sent any data
    last_send_time: NetworkInstant,

    /// Last time we sent a keep-alive
    last_keepalive_time: NetworkInstant,

    /// Current retry count for pending messages
    retry_count: u32,

    /// Start time of current monitoring session
    session_start: NetworkInstant,

    /// Connection health status
    health_status: ConnectionHealth,
}

impl ConnectionMonitor {
    /// Create a new connection monitor
    pub fn new(config: TimeoutConfig) -> Self {
        let now = NetworkInstant::now();

        Self {
            config,
            last_receive_time: now,
            last_send_time: now,
            last_keepalive_time: now,
            retry_count: 0,
            session_start: now,
            health_status: ConnectionHealth::Healthy,
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(TimeoutConfig::default())
    }

    /// Update receive timestamp
    pub fn record_receive(&mut self) {
        self.last_receive_time = NetworkInstant::now();
        self.retry_count = 0; // Reset retry count on successful receive

        // Update health status
        if self.health_status != ConnectionHealth::Healthy {
            debug!("Connection health recovered");
            self.health_status = ConnectionHealth::Healthy;
        }
    }

    /// Update send timestamp
    pub fn record_send(&mut self) {
        self.last_send_time = NetworkInstant::now();
    }

    /// Record a keep-alive sent
    pub fn record_keepalive(&mut self) {
        self.last_keepalive_time = NetworkInstant::now();
    }

    /// Record a retry attempt
    pub fn record_retry(&mut self) -> NetworkResult<()> {
        self.retry_count += 1;

        if self.retry_count >= self.config.max_retries {
            warn!(
                "Maximum retries ({}) exceeded for connection",
                self.config.max_retries
            );
            self.health_status = ConnectionHealth::Dead;
            return Err(NetworkError::connection("maximum retries exceeded"));
        }

        if self.retry_count > self.config.max_retries / 2 {
            self.health_status = ConnectionHealth::Degraded;
        }

        Ok(())
    }

    /// Reset retry counter (call when ack received)
    pub fn reset_retries(&mut self) {
        self.retry_count = 0;
    }

    /// Check if connection has timed out
    pub fn check_timeout(&self) -> NetworkResult<()> {
        let time_since_receive = self.last_receive_time.elapsed();

        if time_since_receive > self.config.connection_timeout {
            warn!(
                "Connection timeout: no data received for {:?}",
                time_since_receive
            );
            return Err(NetworkError::connection(format!(
                "no data received for {:?}",
                time_since_receive
            )));
        }

        Ok(())
    }

    /// Check if keep-alive should be sent
    pub fn should_send_keepalive(&self) -> bool {
        self.last_keepalive_time.elapsed() >= self.config.keepalive_interval
    }

    /// Check if connection is idle
    pub fn is_idle(&self) -> bool {
        let send_idle = self.last_send_time.elapsed() >= self.config.idle_threshold;
        let receive_idle = self.last_receive_time.elapsed() >= self.config.idle_threshold;

        send_idle && receive_idle
    }

    /// Get time since last receive
    pub fn time_since_receive(&self) -> Duration {
        self.last_receive_time.elapsed()
    }

    /// Get time since last send
    pub fn time_since_send(&self) -> Duration {
        self.last_send_time.elapsed()
    }

    /// Get current retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Duration {
        self.session_start.elapsed()
    }

    /// Get current health status
    pub fn health_status(&self) -> ConnectionHealth {
        self.health_status
    }

    /// Check if connection is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.health_status, ConnectionHealth::Healthy)
    }

    /// Check if connection is dead
    pub fn is_dead(&self) -> bool {
        matches!(self.health_status, ConnectionHealth::Dead)
    }

    /// Get comprehensive health report
    pub fn health_report(&self) -> HealthReport {
        HealthReport {
            status: self.health_status,
            uptime: self.uptime(),
            time_since_receive: self.time_since_receive(),
            time_since_send: self.time_since_send(),
            retry_count: self.retry_count,
            is_idle: self.is_idle(),
            should_send_keepalive: self.should_send_keepalive(),
        }
    }

    /// Reset the monitor (for connection re-establishment)
    pub fn reset(&mut self) {
        let now = NetworkInstant::now();
        self.last_receive_time = now;
        self.last_send_time = now;
        self.last_keepalive_time = now;
        self.retry_count = 0;
        self.session_start = now;
        self.health_status = ConnectionHealth::Healthy;
    }
}

/// Connection health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHealth {
    /// Connection is healthy
    Healthy,
    /// Connection is degraded (high retries, latency)
    Degraded,
    /// Connection is dead or unresponsive
    Dead,
}

/// Comprehensive health report
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub status: ConnectionHealth,
    pub uptime: Duration,
    pub time_since_receive: Duration,
    pub time_since_send: Duration,
    pub retry_count: u32,
    pub is_idle: bool,
    pub should_send_keepalive: bool,
}

impl HealthReport {
    /// Check if any issues need attention
    pub fn needs_attention(&self) -> bool {
        matches!(
            self.status,
            ConnectionHealth::Degraded | ConnectionHealth::Dead
        ) || self.retry_count > 3
            || self.time_since_receive > Duration::from_secs(15)
    }
}

/// Bandwidth monitor and limiter
#[derive(Debug)]
pub struct BandwidthMonitor {
    /// Maximum upload bandwidth in bytes per second (0 = unlimited)
    max_upload_bps: u64,

    /// Maximum download bandwidth in bytes per second (0 = unlimited)
    max_download_bps: u64,

    /// Bytes sent in current window
    bytes_sent_window: u64,

    /// Bytes received in current window
    bytes_received_window: u64,

    /// Window start time
    window_start: NetworkInstant,

    /// Window duration
    window_duration: Duration,

    /// Total bytes sent (lifetime)
    total_bytes_sent: u64,

    /// Total bytes received (lifetime)
    total_bytes_received: u64,

    /// Session start time
    session_start: NetworkInstant,
}

impl BandwidthMonitor {
    /// Create a new bandwidth monitor
    pub fn new(max_upload_bps: u64, max_download_bps: u64) -> Self {
        let now = NetworkInstant::now();

        Self {
            max_upload_bps,
            max_download_bps,
            bytes_sent_window: 0,
            bytes_received_window: 0,
            window_start: now,
            window_duration: Duration::from_secs(1),
            total_bytes_sent: 0,
            total_bytes_received: 0,
            session_start: now,
        }
    }

    /// Create unlimited bandwidth monitor
    pub fn unlimited() -> Self {
        Self::new(0, 0)
    }

    /// Record bytes sent
    pub fn record_send(&mut self, bytes: u64) -> NetworkResult<()> {
        self.rotate_window_if_needed();

        // Check bandwidth limit
        if self.max_upload_bps > 0 {
            // Use elapsed time or 1 second as the divisor to match window duration
            let elapsed = self.window_start.elapsed().as_secs_f64();
            let time_for_projection = elapsed.max(1.0); // Assume full 1-second window if early
            let projected_bps = (self.bytes_sent_window + bytes) as f64 / time_for_projection;

            if projected_bps > self.max_upload_bps as f64 {
                return Err(NetworkError::generic("upload bandwidth limit exceeded"));
            }
        }

        self.bytes_sent_window += bytes;
        self.total_bytes_sent += bytes;
        Ok(())
    }

    /// Record bytes received
    pub fn record_receive(&mut self, bytes: u64) -> NetworkResult<()> {
        self.rotate_window_if_needed();

        // Check bandwidth limit
        if self.max_download_bps > 0 {
            // Use elapsed time or 1 second as the divisor to match window duration
            let elapsed = self.window_start.elapsed().as_secs_f64();
            let time_for_projection = elapsed.max(1.0); // Assume full 1-second window if early
            let projected_bps = (self.bytes_received_window + bytes) as f64 / time_for_projection;

            if projected_bps > self.max_download_bps as f64 {
                return Err(NetworkError::generic("download bandwidth limit exceeded"));
            }
        }

        self.bytes_received_window += bytes;
        self.total_bytes_received += bytes;
        Ok(())
    }

    /// Rotate window if duration elapsed
    fn rotate_window_if_needed(&mut self) {
        if self.window_start.elapsed() >= self.window_duration {
            self.bytes_sent_window = 0;
            self.bytes_received_window = 0;
            self.window_start = NetworkInstant::now();
        }
    }

    /// Get current upload rate in bytes per second
    pub fn upload_rate_bps(&self) -> f64 {
        let elapsed = self.window_start.elapsed().as_secs_f64().max(0.001);
        self.bytes_sent_window as f64 / elapsed
    }

    /// Get current download rate in bytes per second
    pub fn download_rate_bps(&self) -> f64 {
        let elapsed = self.window_start.elapsed().as_secs_f64().max(0.001);
        self.bytes_received_window as f64 / elapsed
    }

    /// Get average upload rate over session
    pub fn average_upload_rate_bps(&self) -> f64 {
        let elapsed = self.session_start.elapsed().as_secs_f64().max(0.001);
        self.total_bytes_sent as f64 / elapsed
    }

    /// Get average download rate over session
    pub fn average_download_rate_bps(&self) -> f64 {
        let elapsed = self.session_start.elapsed().as_secs_f64().max(0.001);
        self.total_bytes_received as f64 / elapsed
    }

    /// Get total bytes sent
    pub fn total_bytes_sent(&self) -> u64 {
        self.total_bytes_sent
    }

    /// Get total bytes received
    pub fn total_bytes_received(&self) -> u64 {
        self.total_bytes_received
    }

    /// Check if upload bandwidth is available
    pub fn can_send(&self, bytes: u64) -> bool {
        if self.max_upload_bps == 0 {
            return true;
        }

        let projected_bps = (self.bytes_sent_window + bytes) as f64
            / self.window_start.elapsed().as_secs_f64().max(0.001);

        projected_bps <= self.max_upload_bps as f64
    }

    /// Check if download bandwidth is available
    pub fn can_receive(&self, bytes: u64) -> bool {
        if self.max_download_bps == 0 {
            return true;
        }

        let projected_bps = (self.bytes_received_window + bytes) as f64
            / self.window_start.elapsed().as_secs_f64().max(0.001);

        projected_bps <= self.max_download_bps as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = ConnectionMonitor::default();
        assert!(monitor.is_healthy());
        assert_eq!(monitor.retry_count(), 0);
    }

    #[test]
    fn test_timeout_detection() {
        let mut config = TimeoutConfig::default();
        config.connection_timeout = Duration::from_millis(100);

        let monitor = ConnectionMonitor::new(config);

        // Should not timeout immediately
        assert!(monitor.check_timeout().is_ok());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));
        assert!(monitor.check_timeout().is_err());
    }

    #[test]
    fn test_retry_tracking() {
        let mut monitor = ConnectionMonitor::default();

        // First few retries should succeed
        for i in 0..3 {
            assert!(monitor.record_retry().is_ok());
            assert_eq!(monitor.retry_count(), i + 1);
        }

        // Should mark as degraded
        assert_eq!(monitor.health_status(), ConnectionHealth::Degraded);

        // Max retries should fail
        while monitor.retry_count() < monitor.config.max_retries {
            let _ = monitor.record_retry();
        }
        assert!(monitor.record_retry().is_err());
        assert!(monitor.is_dead());
    }

    #[test]
    fn test_keepalive_timing() {
        let mut config = TimeoutConfig::default();
        config.keepalive_interval = Duration::from_millis(50);

        let monitor = ConnectionMonitor::new(config);

        // Should not need keepalive immediately
        assert!(!monitor.should_send_keepalive());

        // Wait for keepalive interval
        std::thread::sleep(Duration::from_millis(60));
        assert!(monitor.should_send_keepalive());
    }

    #[test]
    fn test_idle_detection() {
        let mut config = TimeoutConfig::default();
        config.idle_threshold = Duration::from_millis(50);

        let monitor = ConnectionMonitor::new(config);

        // Should not be idle immediately
        assert!(!monitor.is_idle());

        // Wait for idle threshold
        std::thread::sleep(Duration::from_millis(60));
        assert!(monitor.is_idle());
    }

    #[test]
    fn test_bandwidth_unlimited() {
        let mut monitor = BandwidthMonitor::unlimited();

        // Should allow any amount
        assert!(monitor.record_send(1_000_000_000).is_ok());
        assert!(monitor.record_receive(1_000_000_000).is_ok());
    }

    #[test]
    fn test_bandwidth_limiting() {
        // 1000 bytes per second limit
        let mut monitor = BandwidthMonitor::new(1000, 1000);

        // Should allow small sends
        assert!(monitor.record_send(500).is_ok());
        assert!(monitor.record_send(400).is_ok());

        // Should reject over limit
        assert!(monitor.record_send(200).is_err());
    }

    #[test]
    fn test_bandwidth_rate_calculation() {
        let mut monitor = BandwidthMonitor::unlimited();

        monitor.record_send(1000).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        // Rate should be approximately 10000 bytes/sec (1000 bytes in 0.1 sec)
        let rate = monitor.upload_rate_bps();
        assert!(rate > 8000.0 && rate < 12000.0);
    }

    #[test]
    fn test_health_report() {
        let monitor = ConnectionMonitor::default();
        let report = monitor.health_report();

        assert_eq!(report.status, ConnectionHealth::Healthy);
        assert!(!report.needs_attention());
        assert_eq!(report.retry_count, 0);
    }

    #[test]
    fn test_monitor_reset() {
        let mut config = TimeoutConfig::default();
        config.idle_threshold = Duration::from_millis(10);

        let mut monitor = ConnectionMonitor::new(config);

        std::thread::sleep(Duration::from_millis(20));
        assert!(monitor.is_idle());

        monitor.reset();
        assert!(!monitor.is_idle());
        assert!(monitor.is_healthy());
        assert_eq!(monitor.retry_count(), 0);
    }
}
