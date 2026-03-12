//! Network security features including rate limiting and DDoS protection
//!
//! This module provides comprehensive network-level security including rate limiting,
//! DDoS detection and mitigation, connection throttling, and IP-based access control.

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use log;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Network security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSecurityConfig {
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Enable DDoS protection
    pub enable_ddos_protection: bool,
    /// Enable IP-based access control
    pub enable_ip_access_control: bool,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
    /// DDoS protection configuration
    pub ddos_protection: DDoSProtectionConfig,
    /// Connection limits
    pub connection_limits: ConnectionLimitsConfig,
    /// Blacklist/whitelist configuration
    pub access_control: AccessControlConfig,
}

impl Default for NetworkSecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            enable_ddos_protection: true,
            enable_ip_access_control: true,
            rate_limiting: RateLimitConfig::default(),
            ddos_protection: DDoSProtectionConfig::default(),
            connection_limits: ConnectionLimitsConfig::default(),
            access_control: AccessControlConfig::default(),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per second per IP
    pub requests_per_second_per_ip: u32,
    /// Requests per second per player
    pub requests_per_second_per_player: u32,
    /// Burst capacity per IP
    pub burst_capacity_per_ip: u32,
    /// Burst capacity per player
    pub burst_capacity_per_player: u32,
    /// Rate limit window duration (seconds)
    pub window_duration_seconds: u64,
    /// Rate limit violation threshold for blocking
    pub violation_threshold: u32,
    /// Block duration for rate limit violations (seconds)
    pub block_duration_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second_per_ip: 100,
            requests_per_second_per_player: 50,
            burst_capacity_per_ip: 200,
            burst_capacity_per_player: 100,
            window_duration_seconds: 60,
            violation_threshold: 5,
            block_duration_seconds: 300, // 5 minutes
        }
    }
}

/// DDoS protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DDoSProtectionConfig {
    /// Maximum connections per IP
    pub max_connections_per_ip: u32,
    /// Connection rate threshold (connections per minute)
    pub connection_rate_threshold: u32,
    /// Packet rate threshold (packets per second)
    pub packet_rate_threshold: u32,
    /// Bandwidth threshold (bytes per second)
    pub bandwidth_threshold: u64,
    /// Anomaly detection sensitivity (0.0 - 1.0)
    pub anomaly_sensitivity: f64,
    /// Auto-block duration for DDoS (seconds)
    pub auto_block_duration_seconds: u64,
}

impl Default for DDoSProtectionConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip: 10,
            connection_rate_threshold: 30, // 30 connections per minute
            packet_rate_threshold: 1000,   // 1000 packets per second
            bandwidth_threshold: 10_000_000, // 10MB/s
            anomaly_sensitivity: 0.8,
            auto_block_duration_seconds: 3600, // 1 hour
        }
    }
}

/// Connection limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLimitsConfig {
    /// Maximum total concurrent connections
    pub max_total_connections: u32,
    /// Maximum connections per subnet (CIDR)
    pub max_connections_per_subnet: u32,
    /// Subnet mask for grouping IPs
    pub subnet_mask_bits: u32,
    /// Connection timeout (seconds)
    pub connection_timeout_seconds: u64,
    /// Idle timeout (seconds)
    pub idle_timeout_seconds: u64,
}

impl Default for ConnectionLimitsConfig {
    fn default() -> Self {
        Self {
            max_total_connections: 1000,
            max_connections_per_subnet: 100,
            subnet_mask_bits: 24, // /24 subnet
            connection_timeout_seconds: 30,
            idle_timeout_seconds: 300, // 5 minutes
        }
    }
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Enable IP whitelist mode (only whitelisted IPs allowed)
    pub whitelist_mode: bool,
    /// Enable IP blacklist
    pub enable_blacklist: bool,
    /// Enable geographic restrictions
    pub enable_geo_restrictions: bool,
    /// Allowed countries (ISO 3166-1 alpha-2)
    pub allowed_countries: Vec<String>,
    /// Blocked countries
    pub blocked_countries: Vec<String>,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            whitelist_mode: false,
            enable_blacklist: true,
            enable_geo_restrictions: false,
            allowed_countries: Vec::new(),
            blocked_countries: Vec::new(),
        }
    }
}

/// Rate limiting state for an IP or player
#[derive(Debug, Clone)]
struct RateLimitState {
    /// Request timestamps in current window
    requests: VecDeque<NetworkInstant>,
    /// Current token bucket count
    tokens: f64,
    /// Last token bucket update
    last_update: NetworkInstant,
    /// Number of violations
    violations: u32,
    /// Block expiry time
    blocked_until: Option<NetworkInstant>,
}

impl RateLimitState {
    fn new() -> Self {
        Self {
            requests: VecDeque::new(),
            tokens: 0.0,
            last_update: NetworkInstant::now(),
            violations: 0,
            blocked_until: None,
        }
    }

    fn is_blocked(&self) -> bool {
        if let Some(blocked_until) = self.blocked_until {
            NetworkInstant::now() < blocked_until
        } else {
            false
        }
    }
}

/// Connection tracking information
#[derive(Debug, Clone)]
struct ConnectionInfo {
    /// Connection ID
    connection_id: Uuid,
    /// Remote IP address
    remote_addr: IpAddr,
    /// Player ID (if authenticated)
    player_id: Option<u8>,
    /// Connection start time
    connected_at: NetworkInstant,
    /// Last activity time
    last_activity: NetworkInstant,
    /// Total bytes sent
    bytes_sent: u64,
    /// Total bytes received
    bytes_received: u64,
    /// Total packets sent
    packets_sent: u64,
    /// Total packets received
    packets_received: u64,
}

/// DDoS attack detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DDoSDetection {
    /// Detection ID
    pub detection_id: Uuid,
    /// Source IP address
    pub source_ip: IpAddr,
    /// Attack type detected
    pub attack_type: DDoSAttackType,
    /// Detection confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Attack metrics
    pub metrics: AttackMetrics,
    /// Detection timestamp
    pub detected_at: SystemTime,
    /// Recommended action
    pub recommended_action: DDoSAction,
}

/// Types of DDoS attacks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DDoSAttackType {
    /// High volume of connections
    ConnectionFlood,
    /// High packet rate
    PacketFlood,
    /// High bandwidth usage
    BandwidthFlood,
    /// Distributed attack from multiple IPs
    DistributedAttack,
    /// Slowloris-style slow connection attack
    SlowlorisAttack,
    /// UDP flood
    UdpFlood,
    /// TCP SYN flood
    SynFlood,
}

/// Attack metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackMetrics {
    /// Connections per minute
    pub connections_per_minute: u32,
    /// Packets per second
    pub packets_per_second: u32,
    /// Bytes per second
    pub bytes_per_second: u64,
    /// Unique source IPs involved
    pub unique_source_ips: u32,
    /// Attack duration (seconds)
    pub duration_seconds: u64,
}

/// Recommended actions for DDoS mitigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DDoSAction {
    /// Monitor and log
    Monitor,
    /// Rate limit the source
    RateLimit,
    /// Temporarily block the source
    TemporaryBlock { duration_seconds: u64 },
    /// Permanently block the source
    PermanentBlock,
    /// Enable connection throttling
    EnableThrottling,
    /// Notify system administrators
    NotifyAdmin,
}

/// Network security manager
pub struct NetworkSecurityManager {
    /// Configuration
    config: NetworkSecurityConfig,
    /// Rate limiting states per IP
    ip_rate_limits: Arc<RwLock<HashMap<IpAddr, RateLimitState>>>,
    /// Rate limiting states per player
    player_rate_limits: Arc<RwLock<HashMap<u8, RateLimitState>>>,
    /// Active connections
    connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    /// IP whitelist
    ip_whitelist: Arc<RwLock<std::collections::HashSet<IpAddr>>>,
    /// IP blacklist
    ip_blacklist: Arc<RwLock<std::collections::HashSet<IpAddr>>>,
    /// DDoS detections
    ddos_detections: Arc<RwLock<Vec<DDoSDetection>>>,
    /// Attack statistics per IP
    attack_stats: Arc<RwLock<HashMap<IpAddr, AttackMetrics>>>,
}

impl NetworkSecurityManager {
    /// Create new network security manager
    pub fn new() -> Self {
        Self::with_config(NetworkSecurityConfig::default())
    }

    /// Create network security manager with configuration
    pub fn with_config(config: NetworkSecurityConfig) -> Self {
        info!("Network security manager initialized");
        info!("Rate limiting: {}", config.enable_rate_limiting);
        info!("DDoS protection: {}", config.enable_ddos_protection);
        info!("IP access control: {}", config.enable_ip_access_control);

        Self {
            config,
            ip_rate_limits: Arc::new(RwLock::new(HashMap::new())),
            player_rate_limits: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            ip_whitelist: Arc::new(RwLock::new(std::collections::HashSet::new())),
            ip_blacklist: Arc::new(RwLock::new(std::collections::HashSet::new())),
            ddos_detections: Arc::new(RwLock::new(Vec::new())),
            attack_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request should be allowed (rate limiting)
    pub async fn check_request_allowed(&self, remote_addr: IpAddr, player_id: Option<u8>) -> NetworkResult<bool> {
        // Check access control first
        if !self.check_ip_access_allowed(remote_addr).await? {
            return Ok(false);
        }

        if !self.config.enable_rate_limiting {
            return Ok(true);
        }

        // Check IP-based rate limiting
        let ip_allowed = self.check_ip_rate_limit(remote_addr).await?;
        if !ip_allowed {
            return Ok(false);
        }

        // Check player-based rate limiting if player is known
        if let Some(player_id) = player_id {
            let player_allowed = self.check_player_rate_limit(player_id).await?;
            if !player_allowed {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check IP access control
    async fn check_ip_access_allowed(&self, ip: IpAddr) -> NetworkResult<bool> {
        if !self.config.enable_ip_access_control {
            return Ok(true);
        }

        // Check blacklist
        if self.config.access_control.enable_blacklist {
            let blacklist = self.ip_blacklist.read().await;
            if blacklist.contains(&ip) {
                return Ok(false);
            }
        }

        // Check whitelist mode
        if self.config.access_control.whitelist_mode {
            let whitelist = self.ip_whitelist.read().await;
            return Ok(whitelist.contains(&ip));
        }

        Ok(true)
    }

    /// Check IP-based rate limiting
    async fn check_ip_rate_limit(&self, ip: IpAddr) -> NetworkResult<bool> {
        let mut rate_limits = self.ip_rate_limits.write().await;
        let state = rate_limits.entry(ip).or_insert_with(RateLimitState::new);

        if state.is_blocked() {
            return Ok(false);
        }

        let now = NetworkInstant::now();
        let window_duration = Duration::from_secs(self.config.rate_limiting.window_duration_seconds);

        // Clean up old requests
        state.requests.retain(|&request_time| now.duration_since(request_time) < window_duration);

        // Check rate limit
        let requests_in_window = state.requests.len() as u32;
        if requests_in_window >= self.config.rate_limiting.requests_per_second_per_ip {
            state.violations += 1;
            
            // Block if too many violations
            if state.violations >= self.config.rate_limiting.violation_threshold {
                state.blocked_until = Some(now + Duration::from_secs(self.config.rate_limiting.block_duration_seconds));
                warn!("IP {} blocked for rate limit violations", ip);
                return Ok(false);
            }
            
            return Ok(false);
        }

        // Update token bucket
        self.update_token_bucket(state, now, 
                                 self.config.rate_limiting.requests_per_second_per_ip as f64,
                                 self.config.rate_limiting.burst_capacity_per_ip as f64);

        if state.tokens < 1.0 {
            return Ok(false);
        }

        // Allow request
        state.requests.push_back(now);
        state.tokens -= 1.0;

        Ok(true)
    }

    /// Check player-based rate limiting
    async fn check_player_rate_limit(&self, player_id: u8) -> NetworkResult<bool> {
        let mut rate_limits = self.player_rate_limits.write().await;
        let state = rate_limits.entry(player_id).or_insert_with(RateLimitState::new);

        if state.is_blocked() {
            return Ok(false);
        }

        let now = NetworkInstant::now();
        let window_duration = Duration::from_secs(self.config.rate_limiting.window_duration_seconds);

        // Clean up old requests
        state.requests.retain(|&request_time| now.duration_since(request_time) < window_duration);

        // Check rate limit
        let requests_in_window = state.requests.len() as u32;
        if requests_in_window >= self.config.rate_limiting.requests_per_second_per_player {
            state.violations += 1;
            
            if state.violations >= self.config.rate_limiting.violation_threshold {
                state.blocked_until = Some(now + Duration::from_secs(self.config.rate_limiting.block_duration_seconds));
                warn!("Player {} blocked for rate limit violations", player_id);
                return Ok(false);
            }
            
            return Ok(false);
        }

        // Update token bucket
        self.update_token_bucket(state, now,
                                 self.config.rate_limiting.requests_per_second_per_player as f64,
                                 self.config.rate_limiting.burst_capacity_per_player as f64);

        if state.tokens < 1.0 {
            return Ok(false);
        }

        // Allow request
        state.requests.push_back(now);
        state.tokens -= 1.0;

        Ok(true)
    }

    /// Update token bucket for rate limiting
    fn update_token_bucket(&self, state: &mut RateLimitState, now: NetworkInstant, rate: f64, capacity: f64) {
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.tokens = (state.tokens + elapsed * rate).min(capacity);
        state.last_update = now;
    }

    /// Register new connection
    pub async fn register_connection(&self, connection_id: Uuid, remote_addr: IpAddr, player_id: Option<u8>) -> NetworkResult<bool> {
        // Check DDoS protection
        if self.config.enable_ddos_protection {
            if let Some(detection) = self.check_ddos_patterns(remote_addr).await? {
                warn!("DDoS attack detected from {}: {:?}", remote_addr, detection.attack_type);
                
                // Auto-block if severe
                let should_block = detection.confidence > 0.9;
                
                // Store detection
                {
                    let mut detections = self.ddos_detections.write().await;
                    detections.push(detection);
                }
                
                if should_block {
                    self.add_to_blacklist(remote_addr).await;
                }
                
                return Ok(false);
            }
        }

        // Check connection limits
        let connections_count = {
            let connections = self.connections.read().await;
            connections.values()
                .filter(|conn| conn.remote_addr == remote_addr)
                .count() as u32
        };

        if connections_count >= self.config.ddos_protection.max_connections_per_ip {
            warn!("Connection limit exceeded for IP {}: {} connections", remote_addr, connections_count);
            return Ok(false);
        }

        // Register connection
        let connection_info = ConnectionInfo {
            connection_id,
            remote_addr,
            player_id,
            connected_at: NetworkInstant::now(),
            last_activity: NetworkInstant::now(),
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
        };

        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, connection_info);
        }

        debug!("Registered connection {} from {}", connection_id, remote_addr);
        Ok(true)
    }

    /// Update connection activity
    pub async fn update_connection_activity(&self, connection_id: Uuid, bytes_sent: u64, bytes_received: u64, packets_sent: u64, packets_received: u64) -> NetworkResult<()> {
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.last_activity = NetworkInstant::now();
            connection.bytes_sent += bytes_sent;
            connection.bytes_received += bytes_received;
            connection.packets_sent += packets_sent;
            connection.packets_received += packets_received;
        }

        Ok(())
    }

    /// Check for DDoS attack patterns
    async fn check_ddos_patterns(&self, remote_addr: IpAddr) -> NetworkResult<Option<DDoSDetection>> {
        let connections = self.connections.read().await;
        let now = NetworkInstant::now();

        // Count recent connections from this IP
        let recent_connections = connections.values()
            .filter(|conn| {
                conn.remote_addr == remote_addr && 
                now.duration_since(conn.connected_at) < Duration::from_secs(60)
            })
            .count() as u32;

        // Check connection rate threshold
        if recent_connections > self.config.ddos_protection.connection_rate_threshold {
            let metrics = AttackMetrics {
                connections_per_minute: recent_connections,
                packets_per_second: 0,
                bytes_per_second: 0,
                unique_source_ips: 1,
                duration_seconds: 60,
            };

            return Ok(Some(DDoSDetection {
                detection_id: Uuid::new_v4(),
                source_ip: remote_addr,
                attack_type: DDoSAttackType::ConnectionFlood,
                confidence: (recent_connections as f64 / self.config.ddos_protection.connection_rate_threshold as f64 - 1.0).min(1.0),
                metrics,
                detected_at: SystemTime::now(),
                recommended_action: if recent_connections > self.config.ddos_protection.connection_rate_threshold * 2 {
                    DDoSAction::TemporaryBlock { duration_seconds: self.config.ddos_protection.auto_block_duration_seconds }
                } else {
                    DDoSAction::RateLimit
                },
            }));
        }

        let mut earliest = now;
        let mut total_packets = 0u64;
        let mut total_bytes = 0u64;
        for conn in connections.values().filter(|conn| conn.remote_addr == remote_addr) {
            total_packets = total_packets.saturating_add(conn.packets_received);
            total_bytes = total_bytes.saturating_add(conn.bytes_received);
            if conn.connected_at < earliest {
                earliest = conn.connected_at;
            }
        }

        let duration_seconds = now
            .duration_since(earliest)
            .as_secs_f64()
            .max(1.0);
        let packets_per_second = (total_packets as f64 / duration_seconds).round() as u32;
        let bytes_per_second = (total_bytes as f64 / duration_seconds).round() as u64;

        if packets_per_second > self.config.ddos_protection.packet_rate_threshold {
            let metrics = AttackMetrics {
                connections_per_minute: recent_connections,
                packets_per_second,
                bytes_per_second,
                unique_source_ips: 1,
                duration_seconds: duration_seconds as u64,
            };

            return Ok(Some(DDoSDetection {
                detection_id: Uuid::new_v4(),
                source_ip: remote_addr,
                attack_type: DDoSAttackType::PacketFlood,
                confidence: (packets_per_second as f64
                    / self.config.ddos_protection.packet_rate_threshold as f64
                    - 1.0)
                    .min(1.0),
                metrics,
                detected_at: SystemTime::now(),
                recommended_action: DDoSAction::RateLimit,
            }));
        }

        if bytes_per_second > self.config.ddos_protection.bandwidth_threshold {
            let metrics = AttackMetrics {
                connections_per_minute: recent_connections,
                packets_per_second,
                bytes_per_second,
                unique_source_ips: 1,
                duration_seconds: duration_seconds as u64,
            };

            return Ok(Some(DDoSDetection {
                detection_id: Uuid::new_v4(),
                source_ip: remote_addr,
                attack_type: DDoSAttackType::BandwidthFlood,
                confidence: (bytes_per_second as f64
                    / self.config.ddos_protection.bandwidth_threshold as f64
                    - 1.0)
                    .min(1.0),
                metrics,
                detected_at: SystemTime::now(),
                recommended_action: DDoSAction::RateLimit,
            }));
        }

        Ok(None)
    }

    /// Add IP to whitelist
    pub async fn add_to_whitelist(&self, ip: IpAddr) {
        let mut whitelist = self.ip_whitelist.write().await;
        whitelist.insert(ip);
        info!("Added {} to IP whitelist", ip);
    }

    /// Add IP to blacklist
    pub async fn add_to_blacklist(&self, ip: IpAddr) {
        let mut blacklist = self.ip_blacklist.write().await;
        blacklist.insert(ip);
        warn!("Added {} to IP blacklist", ip);
    }

    /// Remove IP from blacklist
    pub async fn remove_from_blacklist(&self, ip: IpAddr) -> bool {
        let mut blacklist = self.ip_blacklist.write().await;
        let removed = blacklist.remove(&ip);
        if removed {
            info!("Removed {} from IP blacklist", ip);
        }
        removed
    }

    /// Unregister connection
    pub async fn unregister_connection(&self, connection_id: Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(&connection_id) {
            debug!("Unregistered connection {} from {}", connection_id, connection.remote_addr);
        }
    }

    /// Clean up expired data
    pub async fn cleanup(&self) -> NetworkResult<()> {
        let now = NetworkInstant::now();
        let cleanup_duration = Duration::from_secs(3600); // 1 hour

        // Clean up expired rate limit states
        {
            let mut ip_limits = self.ip_rate_limits.write().await;
            ip_limits.retain(|_, state| {
                !state.requests.is_empty() || state.blocked_until.map_or(false, |blocked| now < blocked)
            });
        }

        {
            let mut player_limits = self.player_rate_limits.write().await;
            player_limits.retain(|_, state| {
                !state.requests.is_empty() || state.blocked_until.map_or(false, |blocked| now < blocked)
            });
        }

        // Clean up idle connections
        {
            let mut connections = self.connections.write().await;
            let idle_timeout = Duration::from_secs(self.config.connection_limits.idle_timeout_seconds);
            connections.retain(|_, conn| now.duration_since(conn.last_activity) < idle_timeout);
        }

        // Clean up old DDoS detections
        {
            let mut detections = self.ddos_detections.write().await;
            let cutoff = SystemTime::now() - Duration::from_secs(86400); // 24 hours
            detections.retain(|detection| detection.detected_at > cutoff);
        }

        Ok(())
    }

    /// Get network security statistics
    pub async fn get_stats(&self) -> NetworkSecurityStats {
        let ip_limits = self.ip_rate_limits.read().await;
        let player_limits = self.player_rate_limits.read().await;
        let connections = self.connections.read().await;
        let whitelist = self.ip_whitelist.read().await;
        let blacklist = self.ip_blacklist.read().await;
        let detections = self.ddos_detections.read().await;

        let blocked_ips = ip_limits.values().filter(|state| state.is_blocked()).count();
        let blocked_players = player_limits.values().filter(|state| state.is_blocked()).count();

        NetworkSecurityStats {
            total_connections: connections.len(),
            blocked_ips,
            blocked_players,
            whitelisted_ips: whitelist.len(),
            blacklisted_ips: blacklist.len(),
            ddos_detections: detections.len(),
            rate_limiting_enabled: self.config.enable_rate_limiting,
            ddos_protection_enabled: self.config.enable_ddos_protection,
        }
    }
}

/// Network security statistics
#[derive(Debug, Clone)]
pub struct NetworkSecurityStats {
    /// Total active connections
    pub total_connections: usize,
    /// Number of blocked IPs
    pub blocked_ips: usize,
    /// Number of blocked players
    pub blocked_players: usize,
    /// Number of whitelisted IPs
    pub whitelisted_ips: usize,
    /// Number of blacklisted IPs
    pub blacklisted_ips: usize,
    /// Number of DDoS detections
    pub ddos_detections: usize,
    /// Whether rate limiting is enabled
    pub rate_limiting_enabled: bool,
    /// Whether DDoS protection is enabled
    pub ddos_protection_enabled: bool,
}

impl Default for NetworkSecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_network_security_manager_creation() {
        let manager = NetworkSecurityManager::new();
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert!(stats.rate_limiting_enabled);
        assert!(stats.ddos_protection_enabled);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = NetworkSecurityConfig {
            rate_limiting: RateLimitConfig {
                requests_per_second_per_ip: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let manager = NetworkSecurityManager::with_config(config);
        let test_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First two requests should be allowed
        assert!(manager.check_request_allowed(test_ip, None).await.unwrap());
        assert!(manager.check_request_allowed(test_ip, None).await.unwrap());

        // Third request should be rate limited
        assert!(!manager.check_request_allowed(test_ip, None).await.unwrap());
    }

    #[tokio::test]
    async fn test_connection_registration() {
        let manager = NetworkSecurityManager::new();
        let connection_id = Uuid::new_v4();
        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        let registered = manager.register_connection(connection_id, test_ip, Some(1)).await.unwrap();
        assert!(registered);

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 1);

        manager.unregister_connection(connection_id).await;
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 0);
    }

    #[tokio::test]
    async fn test_ip_access_control() {
        let manager = NetworkSecurityManager::new();
        let test_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Should be allowed initially
        assert!(manager.check_ip_access_allowed(test_ip).await.unwrap());

        // Add to blacklist
        manager.add_to_blacklist(test_ip).await;
        assert!(!manager.check_ip_access_allowed(test_ip).await.unwrap());

        // Remove from blacklist
        assert!(manager.remove_from_blacklist(test_ip).await);
        assert!(manager.check_ip_access_allowed(test_ip).await.unwrap());
    }
}
