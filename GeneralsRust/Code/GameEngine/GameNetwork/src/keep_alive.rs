//! Keep-Alive System for NAT Mapping and Connection Health
//!
//! This module provides keep-alive functionality to maintain NAT mappings and detect
//! dead connections. It tracks peer activity and sends periodic keep-alive messages
//! to prevent NAT timeouts and detect network failures.
//!
//! # Design
//!
//! The keep-alive system operates on a simple model:
//! - Send keep-alive packets every 15 seconds (KEEP_ALIVE_INTERVAL_SECS) - matches C++ NAT.cpp
//! - Mark peers as dead if no activity for 60 seconds (IDLE_TIMEOUT_SECS)
//! - Track metrics for monitoring and debugging
//!
//! # Integration
//!
//! The keep-alive manager integrates with the transport layer using the
//! `NetCommandType::KeepAlive` command type. The connection manager or transport
//! layer is responsible for:
//! - Calling `should_send_keepalive()` to check if a peer needs a keep-alive
//! - Calling `record_sent()` after sending a keep-alive
//! - Calling `record_received()` when receiving any packet from a peer
//! - Calling `check_timeouts()` periodically to detect dead connections

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

/// Keep-alive interval from C++ constants (15 seconds)
/// MUST match C++ NAT.cpp and transport_udp.rs:34 and lib.rs:178
/// Changed from 20 to 15 seconds to match C++ exactly
pub const KEEP_ALIVE_INTERVAL_SECS: u64 = 15;

/// Idle timeout from C++ constants (60 seconds) - MUST MATCH transport_udp.rs
/// Changed from 30 to 60 seconds for consistency across modules
pub const IDLE_TIMEOUT_SECS: u64 = 60;

/// Keep-alive configuration
#[derive(Debug, Clone)]
pub struct KeepAliveConfig {
    /// Interval between keep-alive sends
    pub interval: Duration,
    /// Timeout after which a peer is considered dead
    pub idle_timeout: Duration,
    /// Payload to send with keep-alive (usually empty)
    pub packet_payload: Vec<u8>,
    /// Whether keep-alive is enabled
    pub enabled: bool,
}

impl Default for KeepAliveConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(KEEP_ALIVE_INTERVAL_SECS),
            idle_timeout: Duration::from_secs(IDLE_TIMEOUT_SECS),
            packet_payload: vec![],
            enabled: true,
        }
    }
}

/// State for a single peer in the keep-alive system
#[derive(Debug, Clone)]
pub struct KeepAliveState {
    /// Peer socket address
    pub address: SocketAddr,
    /// Last time we sent a keep-alive to this peer
    pub last_sent_at: NetworkInstant,
    /// Last time we received any packet from this peer
    pub last_received_at: NetworkInstant,
    /// Total number of keep-alives sent to this peer
    pub sent_count: u64,
    /// Whether this peer is considered alive (within idle timeout)
    pub is_alive: bool,
}

impl KeepAliveState {
    /// Create a new keep-alive state for a peer
    fn new(address: SocketAddr) -> Self {
        let now = NetworkInstant::now();
        Self {
            address,
            last_sent_at: now,
            last_received_at: now,
            sent_count: 0,
            is_alive: true,
        }
    }

    /// Check if we should send a keep-alive to this peer
    fn should_send(&self, interval: Duration) -> bool {
        self.last_sent_at.elapsed() >= interval
    }

    /// Check if this peer has timed out
    fn has_timed_out(&self, timeout: Duration) -> bool {
        self.last_received_at.elapsed() >= timeout
    }

    /// Record that we sent a keep-alive
    fn record_sent(&mut self) {
        self.last_sent_at = NetworkInstant::now();
        self.sent_count += 1;
    }

    /// Record that we received a packet from this peer
    fn record_received(&mut self) {
        self.last_received_at = NetworkInstant::now();
        self.is_alive = true;
    }

    /// Mark this peer as timed out
    fn mark_timed_out(&mut self) {
        self.is_alive = false;
    }
}

/// Keep-alive metrics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct KeepAliveMetrics {
    /// Total keep-alives sent across all peers
    pub total_sent: u64,
    /// Total packets received from all peers
    pub total_received: u64,
    /// Number of peers that have timed out
    pub timeout_count: u64,
    /// Number of currently active peers
    pub active_peers: usize,
    /// Number of currently dead peers
    pub dead_peers: usize,
}

/// Keep-alive manager for tracking peer activity and sending keep-alives
pub struct KeepAliveManager {
    /// Configuration
    config: KeepAliveConfig,
    /// Per-peer keep-alive state
    peers: HashMap<SocketAddr, KeepAliveState>,
    /// Metrics
    metrics: KeepAliveMetrics,
}

impl KeepAliveManager {
    /// Create a new keep-alive manager with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Keep-alive configuration
    ///
    /// # Returns
    ///
    /// A new keep-alive manager instance
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    ///
    /// let config = KeepAliveConfig::default();
    /// let manager = KeepAliveManager::new(config);
    /// ```
    pub fn new(config: KeepAliveConfig) -> Self {
        Self {
            config,
            peers: HashMap::new(),
            metrics: KeepAliveMetrics::default(),
        }
    }

    /// Add a peer to the keep-alive system
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the peer is already registered
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    /// ```
    pub fn add_peer(&mut self, address: SocketAddr) -> NetworkResult<()> {
        if self.peers.contains_key(&address) {
            return Err(NetworkError::connection(format!(
                "Peer {} already registered",
                address
            )));
        }

        let state = KeepAliveState::new(address);
        self.peers.insert(address, state);
        self.update_metrics();

        Ok(())
    }

    /// Remove a peer from the keep-alive system
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer to remove
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    /// manager.remove_peer(addr);
    /// ```
    pub fn remove_peer(&mut self, address: SocketAddr) {
        self.peers.remove(&address);
        self.update_metrics();
    }

    /// Check if a keep-alive should be sent to the specified peer
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer
    ///
    /// # Returns
    ///
    /// `true` if a keep-alive should be sent, `false` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    ///
    /// // Initially, no keep-alive needed (just added)
    /// assert!(!manager.should_send_keepalive(addr));
    /// ```
    pub fn should_send_keepalive(&self, address: SocketAddr) -> bool {
        if !self.config.enabled {
            return false;
        }

        self.peers
            .get(&address)
            .map(|state| state.should_send(self.config.interval))
            .unwrap_or(false)
    }

    /// Record that a keep-alive was sent to the specified peer
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the peer is not registered
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    /// manager.record_sent(addr).unwrap();
    /// ```
    pub fn record_sent(&mut self, address: SocketAddr) -> NetworkResult<()> {
        let state = self
            .peers
            .get_mut(&address)
            .ok_or_else(|| NetworkError::connection(format!("Peer {} not registered", address)))?;

        state.record_sent();
        self.metrics.total_sent += 1;

        Ok(())
    }

    /// Record that a packet was received from the specified peer
    ///
    /// This should be called whenever ANY packet is received from a peer,
    /// not just keep-alive packets. This resets the idle timeout.
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the peer is not registered
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    /// manager.record_received(addr).unwrap();
    /// ```
    pub fn record_received(&mut self, address: SocketAddr) -> NetworkResult<()> {
        let state = self
            .peers
            .get_mut(&address)
            .ok_or_else(|| NetworkError::connection(format!("Peer {} not registered", address)))?;

        state.record_received();
        self.metrics.total_received += 1;

        Ok(())
    }

    /// Get the next peer that needs a keep-alive, if any
    ///
    /// # Returns
    ///
    /// `Some((address, send_time))` if a peer needs a keep-alive,
    /// `None` if no peers need keep-alives
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    ///
    /// // No keep-alive needed immediately
    /// assert!(manager.get_next_keepalive().is_none());
    /// ```
    pub fn get_next_keepalive(&self) -> Option<(SocketAddr, NetworkInstant)> {
        if !self.config.enabled {
            return None;
        }

        self.peers
            .iter()
            .filter(|(_, state)| state.should_send(self.config.interval))
            .min_by_key(|(_, state)| state.last_sent_at)
            .map(|(addr, state)| (*addr, state.last_sent_at))
    }

    /// Check for timed-out peers and return their addresses
    ///
    /// This marks timed-out peers as dead and updates metrics.
    ///
    /// # Returns
    ///
    /// Vector of addresses for peers that have timed out
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    ///
    /// // No timeouts yet
    /// let timed_out = manager.check_timeouts();
    /// assert!(timed_out.is_empty());
    /// ```
    pub fn check_timeouts(&mut self) -> Vec<SocketAddr> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut timed_out = Vec::new();

        for (address, state) in &mut self.peers {
            if state.is_alive && state.has_timed_out(self.config.idle_timeout) {
                state.mark_timed_out();
                timed_out.push(*address);
                self.metrics.timeout_count += 1;
            }
        }

        if !timed_out.is_empty() {
            self.update_metrics();
        }

        timed_out
    }

    /// Get the status of a specific peer
    ///
    /// # Arguments
    ///
    /// * `address` - Socket address of the peer
    ///
    /// # Returns
    ///
    /// `Some(KeepAliveState)` if the peer is registered, `None` otherwise
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    /// use std::net::SocketAddr;
    ///
    /// let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
    /// manager.add_peer(addr).unwrap();
    ///
    /// let status = manager.get_peer_status(addr);
    /// assert!(status.is_some());
    /// assert!(status.unwrap().is_alive);
    /// ```
    pub fn get_peer_status(&self, address: SocketAddr) -> Option<KeepAliveState> {
        self.peers.get(&address).cloned()
    }

    /// Get current metrics
    ///
    /// # Returns
    ///
    /// Current keep-alive metrics
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::keep_alive::{KeepAliveManager, KeepAliveConfig};
    ///
    /// let manager = KeepAliveManager::new(KeepAliveConfig::default());
    /// let metrics = manager.get_metrics();
    /// assert_eq!(metrics.active_peers, 0);
    /// ```
    pub fn get_metrics(&self) -> KeepAliveMetrics {
        self.metrics.clone()
    }

    /// Update metrics based on current state
    fn update_metrics(&mut self) {
        let (active, dead) = self.peers.values().fold((0, 0), |(active, dead), state| {
            if state.is_alive {
                (active + 1, dead)
            } else {
                (active, dead + 1)
            }
        });

        self.metrics.active_peers = active;
        self.metrics.dead_peers = dead;
    }

    /// Get the configured keep-alive interval
    pub fn interval(&self) -> Duration {
        self.config.interval
    }

    /// Get the configured idle timeout
    pub fn idle_timeout(&self) -> Duration {
        self.config.idle_timeout
    }

    /// Check if keep-alive is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set whether keep-alive is enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_default_config() {
        let config = KeepAliveConfig::default();
        assert_eq!(config.interval, Duration::from_secs(15)); // Changed from 20 to 15 to match C++
        assert_eq!(config.idle_timeout, Duration::from_secs(60));
        assert!(config.packet_payload.is_empty());
        assert!(config.enabled);
    }

    #[test]
    fn test_add_remove_peer() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        // Add peer
        assert!(manager.add_peer(addr).is_ok());
        assert_eq!(manager.get_metrics().active_peers, 1);

        // Cannot add twice
        assert!(manager.add_peer(addr).is_err());

        // Remove peer
        manager.remove_peer(addr);
        assert_eq!(manager.get_metrics().active_peers, 0);
    }

    #[test]
    fn test_should_send_keepalive() {
        let mut config = KeepAliveConfig::default();
        config.interval = Duration::from_millis(50);

        let mut manager = KeepAliveManager::new(config);
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // Should not send immediately
        assert!(!manager.should_send_keepalive(addr));

        // Wait for interval
        thread::sleep(Duration::from_millis(60));

        // Should send now
        assert!(manager.should_send_keepalive(addr));

        // Record sent
        manager.record_sent(addr).unwrap();

        // Should not send immediately after
        assert!(!manager.should_send_keepalive(addr));
    }

    #[test]
    fn test_record_sent() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // Record sent
        assert!(manager.record_sent(addr).is_ok());
        assert_eq!(manager.get_metrics().total_sent, 1);

        // Record sent again
        assert!(manager.record_sent(addr).is_ok());
        assert_eq!(manager.get_metrics().total_sent, 2);

        // Get peer status
        let status = manager.get_peer_status(addr).unwrap();
        assert_eq!(status.sent_count, 2);
    }

    #[test]
    fn test_record_received() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // Record received
        assert!(manager.record_received(addr).is_ok());
        assert_eq!(manager.get_metrics().total_received, 1);

        // Record received again
        assert!(manager.record_received(addr).is_ok());
        assert_eq!(manager.get_metrics().total_received, 2);
    }

    #[test]
    fn test_timeout_detection() {
        let mut config = KeepAliveConfig::default();
        config.idle_timeout = Duration::from_millis(50);

        let mut manager = KeepAliveManager::new(config);
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // No timeouts yet
        let timed_out = manager.check_timeouts();
        assert!(timed_out.is_empty());
        assert_eq!(manager.get_metrics().timeout_count, 0);

        // Wait for timeout
        thread::sleep(Duration::from_millis(60));

        // Should time out now
        let timed_out = manager.check_timeouts();
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], addr);
        assert_eq!(manager.get_metrics().timeout_count, 1);
        assert_eq!(manager.get_metrics().active_peers, 0);
        assert_eq!(manager.get_metrics().dead_peers, 1);

        // Peer is marked as dead
        let status = manager.get_peer_status(addr).unwrap();
        assert!(!status.is_alive);
    }

    #[test]
    fn test_timeout_reset_on_receive() {
        let mut config = KeepAliveConfig::default();
        config.idle_timeout = Duration::from_millis(100);

        let mut manager = KeepAliveManager::new(config);
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // Wait half the timeout
        thread::sleep(Duration::from_millis(50));

        // Record received - resets timeout
        manager.record_received(addr).unwrap();

        // Wait another half timeout
        thread::sleep(Duration::from_millis(50));

        // Should NOT time out (total time 100ms, but reset at 50ms)
        let timed_out = manager.check_timeouts();
        assert!(timed_out.is_empty());
        assert_eq!(manager.get_metrics().timeout_count, 0);
    }

    #[test]
    fn test_get_next_keepalive() {
        let mut config = KeepAliveConfig::default();
        config.interval = Duration::from_millis(50);

        let mut manager = KeepAliveManager::new(config);
        let addr1: SocketAddr = "127.0.0.1:8088".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:8089".parse().unwrap();

        manager.add_peer(addr1).unwrap();
        manager.add_peer(addr2).unwrap();

        // No keep-alives needed immediately
        assert!(manager.get_next_keepalive().is_none());

        // Wait for interval
        thread::sleep(Duration::from_millis(60));

        // Should get one of the peers (both need keep-alive)
        let next = manager.get_next_keepalive();
        assert!(next.is_some());
        let (addr, _) = next.unwrap();
        assert!(addr == addr1 || addr == addr2);
    }

    #[test]
    fn test_multi_peer_scenario() {
        let mut config = KeepAliveConfig::default();
        config.interval = Duration::from_millis(50);
        config.idle_timeout = Duration::from_millis(100);

        let mut manager = KeepAliveManager::new(config);

        let addr1: SocketAddr = "127.0.0.1:8088".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:8089".parse().unwrap();
        let addr3: SocketAddr = "127.0.0.1:8090".parse().unwrap();

        // Add three peers
        manager.add_peer(addr1).unwrap();
        manager.add_peer(addr2).unwrap();
        manager.add_peer(addr3).unwrap();

        assert_eq!(manager.get_metrics().active_peers, 3);

        // Wait a bit
        thread::sleep(Duration::from_millis(60));

        // Receive from peer 1 (resets timeout)
        manager.record_received(addr1).unwrap();

        // Receive from peer 2 (resets timeout)
        manager.record_received(addr2).unwrap();

        // Wait for timeout
        thread::sleep(Duration::from_millis(60));

        // Peer 3 should time out (no activity), but not peers 1 and 2 (recent activity)
        let timed_out = manager.check_timeouts();
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], addr3);

        assert_eq!(manager.get_metrics().active_peers, 2);
        assert_eq!(manager.get_metrics().dead_peers, 1);
    }

    #[test]
    fn test_metrics_tracking() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        let metrics = manager.get_metrics();
        assert_eq!(metrics.active_peers, 1);
        assert_eq!(metrics.dead_peers, 0);
        assert_eq!(metrics.total_sent, 0);
        assert_eq!(metrics.total_received, 0);
        assert_eq!(metrics.timeout_count, 0);

        // Send and receive
        manager.record_sent(addr).unwrap();
        manager.record_sent(addr).unwrap();
        manager.record_received(addr).unwrap();

        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_sent, 2);
        assert_eq!(metrics.total_received, 1);
    }

    #[test]
    fn test_disabled_keepalive() {
        let mut config = KeepAliveConfig::default();
        config.enabled = false;

        let mut manager = KeepAliveManager::new(config);
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        manager.add_peer(addr).unwrap();

        // Should not send when disabled
        assert!(!manager.should_send_keepalive(addr));

        // Get next should return None
        assert!(manager.get_next_keepalive().is_none());

        // Check timeouts should return empty
        assert!(manager.check_timeouts().is_empty());
    }

    #[test]
    fn test_enable_disable() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        assert!(manager.is_enabled());

        manager.set_enabled(false);
        assert!(!manager.is_enabled());

        manager.set_enabled(true);
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_unknown_peer() {
        let mut manager = KeepAliveManager::new(KeepAliveConfig::default());
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();

        // Operations on unknown peer should fail
        assert!(manager.record_sent(addr).is_err());
        assert!(manager.record_received(addr).is_err());

        // Query should return None
        assert!(manager.get_peer_status(addr).is_none());

        // Should send returns false for unknown peer
        assert!(!manager.should_send_keepalive(addr));
    }

    #[test]
    fn test_peer_state() {
        let addr: SocketAddr = "127.0.0.1:8088".parse().unwrap();
        let state = KeepAliveState::new(addr);

        assert_eq!(state.address, addr);
        assert_eq!(state.sent_count, 0);
        assert!(state.is_alive);
        assert!(!state.has_timed_out(Duration::from_secs(1)));
    }
}
