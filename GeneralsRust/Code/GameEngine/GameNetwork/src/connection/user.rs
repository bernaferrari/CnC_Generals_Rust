//! User/Player session management
//!
//! This module implements player session tracking matching the C++ original's
//! User class and adds comprehensive session lifecycle management.

use crate::time::NetworkInstant;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

/// User/Player session representing a connected player
///
/// This matches the C++ User class but with additional session management fields
#[derive(Debug, Clone)]
pub struct User {
    /// Player's display name (Unicode string in C++ original)
    pub name: String,

    /// Player's socket address (combines IP and port from C++)
    pub addr: SocketAddr,

    /// Player ID (0-7 for 8-player games)
    pub player_id: u8,

    /// Session start time
    pub session_start: DateTime<Utc>,

    /// Last time we heard from this user
    pub last_activity: NetworkInstant,

    /// Last time we sent data to this user
    pub last_send: NetworkInstant,

    /// Session state
    pub state: UserState,

    /// Network statistics
    pub stats: UserNetworkStats,

    /// User preferences/settings
    pub settings: UserSettings,

    /// Authentication/verification data
    pub auth: UserAuth,
}

impl User {
    /// Create a new user session
    pub fn new(name: String, addr: SocketAddr, player_id: u8) -> Self {
        let now = NetworkInstant::now();

        Self {
            name,
            addr,
            player_id,
            session_start: Utc::now(),
            last_activity: now,
            last_send: now,
            state: UserState::Connected,
            stats: UserNetworkStats::default(),
            settings: UserSettings::default(),
            auth: UserAuth::default(),
        }
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = NetworkInstant::now();
    }

    /// Update last send timestamp
    pub fn update_send(&mut self) {
        self.last_send = NetworkInstant::now();
    }

    /// Check if user has timed out
    pub fn has_timed_out(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Get session duration
    pub fn session_duration(&self) -> Duration {
        Utc::now()
            .signed_duration_since(self.session_start)
            .to_std()
            .unwrap_or_default()
    }

    /// Get time since last activity
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Check if user is active (not idle)
    pub fn is_active(&self, idle_threshold: Duration) -> bool {
        self.idle_time() < idle_threshold
    }

    /// Update network statistics
    pub fn update_stats(&mut self, bytes_sent: u64, bytes_received: u64, latency_ms: f64) {
        self.stats.bytes_sent += bytes_sent;
        self.stats.bytes_received += bytes_received;
        self.stats.packets_sent += if bytes_sent > 0 { 1 } else { 0 };
        self.stats.packets_received += if bytes_received > 0 { 1 } else { 0 };

        // Update latency with exponential moving average
        if self.stats.average_latency_ms == 0.0 {
            self.stats.average_latency_ms = latency_ms;
        } else {
            self.stats.average_latency_ms = self.stats.average_latency_ms * 0.9 + latency_ms * 0.1;
        }

        self.stats.current_latency_ms = latency_ms;

        // Calculate bandwidth (bytes per second)
        let session_seconds = self.session_duration().as_secs_f64().max(1.0);
        self.stats.upload_bandwidth_bps = (self.stats.bytes_sent as f64 / session_seconds) * 8.0;
        self.stats.download_bandwidth_bps =
            (self.stats.bytes_received as f64 / session_seconds) * 8.0;
    }

    /// Calculate packet loss rate
    pub fn packet_loss_rate(&self) -> f32 {
        if self.stats.packets_sent == 0 {
            return 0.0;
        }

        let lost = self
            .stats
            .packets_sent
            .saturating_sub(self.stats.packets_acknowledged);

        (lost as f32 / self.stats.packets_sent as f32) * 100.0
    }

    /// Get connection quality assessment
    pub fn connection_quality(&self) -> ConnectionQuality {
        let latency = self.stats.current_latency_ms;
        let loss_rate = self.packet_loss_rate();

        match (latency, loss_rate) {
            (l, p) if l <= 50.0 && p < 1.0 => ConnectionQuality::Excellent,
            (l, p) if l <= 100.0 && p < 5.0 => ConnectionQuality::Good,
            (l, p) if l <= 200.0 && p < 10.0 => ConnectionQuality::Fair,
            _ => ConnectionQuality::Poor,
        }
    }

    /// Check if user is compatible (version, mods, etc.)
    pub fn is_compatible(&self, required_version: Option<&str>, required_crc: Option<u32>) -> bool {
        // Check version
        if let (Some(their_ver), Some(req_ver)) = (&self.auth.game_version, required_version) {
            if their_ver != req_ver {
                return false;
            }
        }

        // Check executable CRC
        if let (Some(their_crc), Some(req_crc)) = (self.auth.exe_crc, required_crc) {
            if their_crc != req_crc {
                return false;
            }
        }

        true
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "User {{ name: {}, id: {}, addr: {}, state: {}, latency: {:.1}ms }}",
            self.name, self.player_id, self.addr, self.state, self.stats.current_latency_ms
        )
    }
}

/// User session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserState {
    /// Initial connection established
    Connected,
    /// User authenticated
    Authenticated,
    /// User in lobby
    InLobby,
    /// User loading game
    Loading,
    /// User ready to play
    Ready,
    /// User actively playing
    InGame,
    /// User paused/inactive
    Inactive,
    /// User disconnecting
    Disconnecting,
    /// User disconnected
    Disconnected,
}

impl fmt::Display for UserState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Connected => "Connected",
            Self::Authenticated => "Authenticated",
            Self::InLobby => "InLobby",
            Self::Loading => "Loading",
            Self::Ready => "Ready",
            Self::InGame => "InGame",
            Self::Inactive => "Inactive",
            Self::Disconnecting => "Disconnecting",
            Self::Disconnected => "Disconnected",
        };
        write!(f, "{}", s)
    }
}

/// Connection quality assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Unknown,
}

/// Network statistics for a user
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserNetworkStats {
    /// Total bytes sent to this user
    pub bytes_sent: u64,
    /// Total bytes received from this user
    pub bytes_received: u64,
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets received
    pub packets_received: u64,
    /// Packets acknowledged (for reliability tracking)
    pub packets_acknowledged: u64,
    /// Current latency in milliseconds
    pub current_latency_ms: f64,
    /// Average latency in milliseconds (matches C++ m_averageLatency)
    pub average_latency_ms: f64,
    /// Upload bandwidth in bits per second
    pub upload_bandwidth_bps: f64,
    /// Download bandwidth in bits per second
    pub download_bandwidth_bps: f64,
    /// Number of retransmissions
    pub retransmissions: u64,
}

impl UserNetworkStats {
    /// Record a packet acknowledgment
    pub fn record_ack(&mut self) {
        self.packets_acknowledged += 1;
    }

    /// Record a retransmission
    pub fn record_retransmission(&mut self) {
        self.retransmissions += 1;
    }

    /// Get efficiency ratio (acks / sent)
    pub fn efficiency(&self) -> f32 {
        if self.packets_sent == 0 {
            return 1.0;
        }
        (self.packets_acknowledged as f32 / self.packets_sent as f32).min(1.0)
    }
}

/// User settings/preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// User's preferred color
    pub preferred_color: Option<u32>,
    /// User's preferred faction
    pub preferred_faction: Option<String>,
    /// User's preferred team
    pub preferred_team: Option<u8>,
    /// Maximum allowed latency before warning
    pub max_latency_ms: f64,
    /// Compression enabled
    pub compression_enabled: bool,
    /// Encryption enabled
    pub encryption_enabled: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            preferred_color: None,
            preferred_faction: None,
            preferred_team: None,
            max_latency_ms: 500.0,
            compression_enabled: true,
            encryption_enabled: true,
        }
    }
}

/// User authentication/verification data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserAuth {
    /// Game version string
    pub game_version: Option<String>,
    /// Executable CRC for version checking
    pub exe_crc: Option<u32>,
    /// INI/config CRC for mod compatibility
    pub ini_crc: Option<u32>,
    /// Serial number hash for duplicate checking
    pub serial_hash: Option<String>,
    /// Whether user is authenticated
    pub is_authenticated: bool,
    /// Authentication token (if using secure authentication)
    pub auth_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_user_creation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8088);
        let user = User::new("TestPlayer".to_string(), addr, 0);

        assert_eq!(user.name, "TestPlayer");
        assert_eq!(user.addr, addr);
        assert_eq!(user.player_id, 0);
        assert_eq!(user.state, UserState::Connected);
    }

    #[test]
    fn test_activity_tracking() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        std::thread::sleep(Duration::from_millis(10));

        assert!(user.idle_time() >= Duration::from_millis(10));

        user.update_activity();
        assert!(user.idle_time() < Duration::from_millis(5));
    }

    #[test]
    fn test_timeout_detection() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let user = User::new("Test".to_string(), addr, 0);

        // Should not timeout immediately
        assert!(!user.has_timed_out(Duration::from_secs(30)));

        // Simulate timeout by checking very short duration
        std::thread::sleep(Duration::from_millis(10));
        assert!(user.has_timed_out(Duration::from_millis(5)));
    }

    #[test]
    fn test_stats_update() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        user.update_stats(1000, 500, 50.0);

        assert_eq!(user.stats.bytes_sent, 1000);
        assert_eq!(user.stats.bytes_received, 500);
        assert_eq!(user.stats.packets_sent, 1);
        assert_eq!(user.stats.current_latency_ms, 50.0);
        assert_eq!(user.stats.average_latency_ms, 50.0);
    }

    #[test]
    fn test_latency_averaging() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        user.update_stats(100, 100, 100.0);
        assert_eq!(user.stats.average_latency_ms, 100.0);

        user.update_stats(100, 100, 50.0);
        // Average should be: 100.0 * 0.9 + 50.0 * 0.1 = 95.0
        assert!((user.stats.average_latency_ms - 95.0).abs() < 0.01);
    }

    #[test]
    fn test_packet_loss_calculation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        // Send 100 packets
        for _ in 0..100 {
            user.update_stats(100, 0, 50.0);
        }

        // Acknowledge 95 packets
        for _ in 0..95 {
            user.stats.record_ack();
        }

        let loss_rate = user.packet_loss_rate();
        assert!((loss_rate - 5.0).abs() < 0.1); // ~5% packet loss
    }

    #[test]
    fn test_connection_quality() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        // Excellent connection
        user.update_stats(1000, 1000, 30.0);
        user.stats.packets_acknowledged = user.stats.packets_sent;
        assert_eq!(user.connection_quality(), ConnectionQuality::Excellent);

        // Good connection
        user.stats.current_latency_ms = 80.0;
        assert_eq!(user.connection_quality(), ConnectionQuality::Good);

        // Poor connection
        user.stats.current_latency_ms = 300.0;
        assert_eq!(user.connection_quality(), ConnectionQuality::Poor);
    }

    #[test]
    fn test_compatibility_check() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let mut user = User::new("Test".to_string(), addr, 0);

        user.auth.game_version = Some("1.0.0".to_string());
        user.auth.exe_crc = Some(0x12345678);

        // Compatible
        assert!(user.is_compatible(Some("1.0.0"), Some(0x12345678)));

        // Incompatible version
        assert!(!user.is_compatible(Some("2.0.0"), Some(0x12345678)));

        // Incompatible CRC
        assert!(!user.is_compatible(Some("1.0.0"), Some(0x87654321)));

        // Unknown versions should be compatible
        let user2 = User::new("Test2".to_string(), addr, 1);
        assert!(user2.is_compatible(None, None));
    }

    #[test]
    fn test_user_display() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8088);
        let user = User::new("TestPlayer".to_string(), addr, 5);

        let display = format!("{}", user);
        assert!(display.contains("TestPlayer"));
        assert!(display.contains("id: 5"));
        assert!(display.contains("192.168.1.100"));
    }
}
