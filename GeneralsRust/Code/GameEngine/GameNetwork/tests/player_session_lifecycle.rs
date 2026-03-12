//! Comprehensive tests for player session lifecycle and connection management
//!
//! These tests verify the complete player session lifecycle matching the C++ original:
//! - Player connection and initialization
//! - Session state transitions
//! - Timeout detection
//! - Bandwidth management
//! - Multi-player scenarios
//! - Disconnection handling

use game_network::connection::{
    BandwidthMonitor, Connection, ConnectionConfig, ConnectionManager, ConnectionMonitor,
    ConnectionState, TimeoutConfig, User, UserState,
};
use game_network::error::NetworkResult;
use game_network::transport::{Transport, TransportProtocol};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Helper function to create test address
fn test_addr(last_octet: u8, port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, last_octet)), port)
}

#[tokio::test]
async fn test_player_session_creation() {
    let addr = test_addr(100, 8088);
    let user = User::new("Player1".to_string(), addr, 0);

    assert_eq!(user.name, "Player1");
    assert_eq!(user.addr, addr);
    assert_eq!(user.player_id, 0);
    assert_eq!(user.state, UserState::Connected);
    assert!(user.stats.bytes_sent == 0);
    assert!(user.stats.bytes_received == 0);
}

#[tokio::test]
async fn test_player_session_state_transitions() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Connected -> Authenticated
    user.state = UserState::Authenticated;
    assert_eq!(user.state, UserState::Authenticated);

    // Authenticated -> InLobby
    user.state = UserState::InLobby;
    assert_eq!(user.state, UserState::InLobby);

    // InLobby -> Loading
    user.state = UserState::Loading;
    assert_eq!(user.state, UserState::Loading);

    // Loading -> Ready
    user.state = UserState::Ready;
    assert_eq!(user.state, UserState::Ready);

    // Ready -> InGame
    user.state = UserState::InGame;
    assert_eq!(user.state, UserState::InGame);

    // InGame -> Disconnecting
    user.state = UserState::Disconnecting;
    assert_eq!(user.state, UserState::Disconnecting);

    // Disconnecting -> Disconnected
    user.state = UserState::Disconnected;
    assert_eq!(user.state, UserState::Disconnected);
}

#[tokio::test]
async fn test_connection_timeout_detection() {
    let mut config = TimeoutConfig::default();
    config.connection_timeout = Duration::from_millis(100);

    let monitor = ConnectionMonitor::new(config);

    // Should not timeout immediately
    assert!(monitor.check_timeout().is_ok());
    assert!(monitor.is_healthy());

    // Wait for timeout
    sleep(Duration::from_millis(150)).await;

    // Should detect timeout
    assert!(monitor.check_timeout().is_err());
}

#[tokio::test]
async fn test_player_activity_tracking() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Initial activity should be recent
    assert!(user.idle_time() < Duration::from_secs(1));

    // Wait a bit
    sleep(Duration::from_millis(50)).await;

    // Idle time should have increased
    assert!(user.idle_time() >= Duration::from_millis(50));

    // Update activity
    user.update_activity();

    // Idle time should be reset
    assert!(user.idle_time() < Duration::from_millis(10));
}

#[tokio::test]
async fn test_bandwidth_tracking() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Send some data
    user.update_stats(1000, 500, 50.0);

    assert_eq!(user.stats.bytes_sent, 1000);
    assert_eq!(user.stats.bytes_received, 500);
    assert_eq!(user.stats.packets_sent, 1);
    assert_eq!(user.stats.packets_received, 1);

    // Send more data
    user.update_stats(2000, 1500, 60.0);

    assert_eq!(user.stats.bytes_sent, 3000);
    assert_eq!(user.stats.bytes_received, 2000);
    assert_eq!(user.stats.packets_sent, 2);
    assert_eq!(user.stats.packets_received, 2);

    // Check bandwidth calculation (should be non-zero)
    sleep(Duration::from_millis(100)).await;
    user.update_stats(1000, 1000, 55.0);

    assert!(user.stats.upload_bandwidth_bps > 0.0);
    assert!(user.stats.download_bandwidth_bps > 0.0);
}

#[tokio::test]
async fn test_bandwidth_limiting() {
    // Create monitor with 10KB/s limit
    let mut monitor = BandwidthMonitor::new(10_000, 10_000);

    // Should allow small sends
    assert!(monitor.record_send(5000).is_ok());

    // Should allow up to limit
    assert!(monitor.record_send(4000).is_ok());

    // Should reject over limit (within same window)
    assert!(monitor.record_send(2000).is_err());

    // Wait for window to rotate
    sleep(Duration::from_millis(1100)).await;

    // Should allow again after window rotation
    assert!(monitor.record_send(5000).is_ok());
}

#[tokio::test]
async fn test_latency_tracking() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Update with initial latency
    user.update_stats(100, 100, 100.0);
    assert_eq!(user.stats.current_latency_ms, 100.0);
    assert_eq!(user.stats.average_latency_ms, 100.0);

    // Update with different latency (should use exponential moving average)
    user.update_stats(100, 100, 50.0);

    assert_eq!(user.stats.current_latency_ms, 50.0);
    // Average should be: 100.0 * 0.9 + 50.0 * 0.1 = 95.0
    assert!((user.stats.average_latency_ms - 95.0).abs() < 0.01);
}

#[tokio::test]
async fn test_packet_loss_tracking() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Send 100 packets
    for _ in 0..100 {
        user.update_stats(1000, 0, 50.0);
    }

    // Acknowledge 90 packets
    for _ in 0..90 {
        user.stats.record_ack();
    }

    let loss_rate = user.packet_loss_rate();

    // Should be ~10% packet loss
    assert!((loss_rate - 10.0).abs() < 1.0);
}

#[tokio::test]
async fn test_connection_quality_assessment() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Test excellent connection (low latency, no loss)
    user.update_stats(1000, 1000, 30.0);
    for _ in 0..user.stats.packets_sent {
        user.stats.record_ack();
    }
    assert_eq!(
        user.connection_quality(),
        game_network::connection::ConnectionQuality::Excellent
    );

    // Test good connection
    user.stats.current_latency_ms = 80.0;
    assert_eq!(
        user.connection_quality(),
        game_network::connection::ConnectionQuality::Good
    );

    // Test fair connection
    user.stats.current_latency_ms = 150.0;
    assert_eq!(
        user.connection_quality(),
        game_network::connection::ConnectionQuality::Fair
    );

    // Test poor connection
    user.stats.current_latency_ms = 300.0;
    assert_eq!(
        user.connection_quality(),
        game_network::connection::ConnectionQuality::Poor
    );
}

#[tokio::test]
async fn test_multi_player_scenario() {
    // Create 8 players
    let mut players = Vec::new();

    for i in 0..8 {
        let addr = test_addr(100 + i, 8088 + i as u16);
        let user = User::new(format!("Player{}", i), addr, i);
        players.push(user);
    }

    assert_eq!(players.len(), 8);

    // Simulate activity for each player
    for player in &mut players {
        player.update_stats(1000, 500, (player.player_id as f64) * 10.0);
        player.update_activity();
    }

    // Verify all players are tracked
    assert!(players
        .iter()
        .all(|p| p.idle_time() < Duration::from_secs(1)));

    // Simulate one player timing out
    players[3].state = UserState::Disconnected;

    // Count active players
    let active_count = players
        .iter()
        .filter(|p| p.state != UserState::Disconnected)
        .count();

    assert_eq!(active_count, 7);
}

#[tokio::test]
async fn test_sudden_disconnect_handling() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Player is initially active
    assert_eq!(user.state, UserState::Connected);

    // Simulate game activity
    user.state = UserState::InGame;
    user.update_stats(5000, 3000, 45.0);

    // Sudden disconnect
    user.state = UserState::Disconnected;

    // Verify disconnect is tracked
    assert_eq!(user.state, UserState::Disconnected);

    // Stats should still be preserved
    assert_eq!(user.stats.bytes_sent, 5000);
    assert_eq!(user.stats.bytes_received, 3000);
}

#[tokio::test]
async fn test_graceful_disconnect() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Transition through states
    user.state = UserState::InGame;

    // Start graceful disconnect
    user.state = UserState::Disconnecting;
    assert_eq!(user.state, UserState::Disconnecting);

    // Complete disconnect
    user.state = UserState::Disconnected;
    assert_eq!(user.state, UserState::Disconnected);
}

#[tokio::test]
async fn test_retry_mechanism() {
    let mut config = TimeoutConfig::default();
    config.max_retries = 5;
    config.ack_timeout = Duration::from_millis(100);

    let mut monitor = ConnectionMonitor::new(config);

    // Should allow retries up to (max_retries - 1); hitting max_retries marks dead.
    for i in 0..4 {
        assert!(monitor.record_retry().is_ok());
        assert_eq!(monitor.retry_count(), i + 1);
    }

    // Should fail on max retries
    assert!(monitor.record_retry().is_err());
    assert!(monitor.is_dead());
}

#[tokio::test]
async fn test_keepalive_mechanism() {
    let mut config = TimeoutConfig::default();
    config.keepalive_interval = Duration::from_millis(100);

    let mut monitor = ConnectionMonitor::new(config);

    // Should not need keepalive immediately
    assert!(!monitor.should_send_keepalive());

    // Wait for keepalive interval
    sleep(Duration::from_millis(150)).await;

    // Should trigger keepalive
    assert!(monitor.should_send_keepalive());

    // After recording keepalive, should reset
    monitor.record_keepalive();
    assert!(!monitor.should_send_keepalive());
}

#[tokio::test]
async fn test_idle_detection() {
    let mut config = TimeoutConfig::default();
    config.idle_threshold = Duration::from_millis(100);

    let monitor = ConnectionMonitor::new(config);

    // Should not be idle immediately
    assert!(!monitor.is_idle());

    // Wait for idle threshold
    sleep(Duration::from_millis(150)).await;

    // Should be idle now
    assert!(monitor.is_idle());
}

#[tokio::test]
async fn test_session_duration_tracking() {
    let addr = test_addr(100, 8088);
    let user = User::new("Player1".to_string(), addr, 0);

    // Session just started
    assert!(user.session_duration() < Duration::from_secs(1));

    // Wait a bit
    sleep(Duration::from_millis(100)).await;

    // Session duration should have increased
    assert!(user.session_duration() >= Duration::from_millis(100));
}

#[tokio::test]
async fn test_compatibility_checking() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Set version and CRC
    user.auth.game_version = Some("1.04".to_string());
    user.auth.exe_crc = Some(0x12345678);

    // Compatible check
    assert!(user.is_compatible(Some("1.04"), Some(0x12345678)));

    // Incompatible version
    assert!(!user.is_compatible(Some("1.05"), Some(0x12345678)));

    // Incompatible CRC
    assert!(!user.is_compatible(Some("1.04"), Some(0x87654321)));

    // Unknown should be compatible
    let user2 = User::new("Player2".to_string(), addr, 1);
    assert!(user2.is_compatible(None, None));
}

#[tokio::test]
async fn test_player_slot_management() {
    let mut players: Vec<Option<User>> = vec![None; 8];

    // Add players to slots
    for i in 0..5 {
        let addr = test_addr(100 + i, 8088 + i as u16);
        let user = User::new(format!("Player{}", i), addr, i);
        players[i as usize] = Some(user);
    }

    // Verify slot occupancy
    let occupied = players.iter().filter(|p| p.is_some()).count();
    assert_eq!(occupied, 5);

    // Remove a player from slot 2
    players[2] = None;

    let occupied = players.iter().filter(|p| p.is_some()).count();
    assert_eq!(occupied, 4);

    // Add new player to slot 2
    let addr = test_addr(102, 8090);
    players[2] = Some(User::new("Player2New".to_string(), addr, 2));

    let occupied = players.iter().filter(|p| p.is_some()).count();
    assert_eq!(occupied, 5);
}

#[tokio::test]
async fn test_connection_health_monitoring() {
    let monitor = ConnectionMonitor::default();

    // Should be healthy initially
    assert!(monitor.is_healthy());
    assert!(!monitor.is_dead());

    let report = monitor.health_report();
    assert_eq!(
        report.status,
        game_network::connection::ConnectionHealth::Healthy
    );
    assert!(!report.needs_attention());
}

#[tokio::test]
async fn test_bandwidth_rate_calculation() {
    let mut monitor = BandwidthMonitor::unlimited();

    // Send 10KB
    monitor.record_send(10_000).unwrap();

    // Wait 100ms
    sleep(Duration::from_millis(100)).await;

    // Rate should be approximately 100 KB/s
    let rate_bps = monitor.upload_rate_bps();
    assert!(rate_bps > 80_000.0 && rate_bps < 120_000.0);
}

#[tokio::test]
async fn test_all_8_players_simultaneous() {
    let mut monitors = Vec::new();
    let mut users = Vec::new();

    // Create 8 players with monitors
    for i in 0..8 {
        let addr = test_addr(100 + i, 8088 + i as u16);
        let user = User::new(format!("Player{}", i), addr, i);
        let monitor = ConnectionMonitor::default();

        users.push(user);
        monitors.push(monitor);
    }

    // Simulate activity for all players
    for i in 0..8 {
        users[i].update_stats(1000 * (i as u64 + 1), 500 * (i as u64 + 1), 50.0);
        monitors[i].record_receive();
        monitors[i].record_send();
    }

    // Verify all players are healthy
    assert_eq!(users.len(), 8);
    assert_eq!(monitors.len(), 8);
    assert!(monitors.iter().all(|m| m.is_healthy()));

    // Verify different bandwidth for each player
    for i in 0..7 {
        assert!(users[i + 1].stats.bytes_sent > users[i].stats.bytes_sent);
    }
}

#[tokio::test]
async fn test_edge_case_instant_disconnect() {
    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);

    // Instant disconnect (without going through states)
    user.state = UserState::Disconnected;

    // Should still have valid session start time
    assert!(user.session_duration() < Duration::from_secs(1));

    // Stats should be zero
    assert_eq!(user.stats.bytes_sent, 0);
    assert_eq!(user.stats.bytes_received, 0);
}

#[tokio::test]
async fn test_edge_case_timeout_during_loading() {
    let mut config = TimeoutConfig::default();
    config.connection_timeout = Duration::from_millis(100);

    let addr = test_addr(100, 8088);
    let mut user = User::new("Player1".to_string(), addr, 0);
    let monitor = ConnectionMonitor::new(config);

    // Player enters loading state
    user.state = UserState::Loading;

    // Wait for timeout
    sleep(Duration::from_millis(150)).await;

    // Monitor should detect timeout
    assert!(monitor.check_timeout().is_err());

    // Player should be marked as having issues
    assert_eq!(user.state, UserState::Loading); // State doesn't auto-change
}
