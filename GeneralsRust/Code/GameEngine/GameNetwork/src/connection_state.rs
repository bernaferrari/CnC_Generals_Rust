//! Connection State Machine - C++ NetLocalStatus Lifecycle
//!
//! This module implements a connection state machine matching the C++ network lifecycle
//! with states: PreGame, InGame, Leaving, Left, and PostGame.
//!
//! Valid state transitions:
//! - PreGame    → InGame
//! - InGame     → Leaving, PostGame
//! - Leaving    → Left
//! - Left       (terminal state)
//! - PostGame   → PreGame (for rematches)

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Connection state matching C++ NetLocalStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    /// Before game starts - initial connection state
    PreGame,
    /// During gameplay - actively playing
    InGame,
    /// Disconnecting - graceful disconnect in progress
    Leaving,
    /// Disconnected - terminal state, connection closed
    Left,
    /// After game ends - waiting for next game or rematch
    PostGame,
}

impl ConnectionState {
    /// Check if state is terminal (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(self, ConnectionState::Left)
    }

    /// Check if state allows game activity
    pub fn allows_game_activity(&self) -> bool {
        matches!(self, ConnectionState::InGame)
    }

    /// Check if connection is active (not left)
    pub fn is_active(&self) -> bool {
        !matches!(self, ConnectionState::Left)
    }
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ConnectionState::PreGame => "PreGame",
            ConnectionState::InGame => "InGame",
            ConnectionState::Leaving => "Leaving",
            ConnectionState::Left => "Left",
            ConnectionState::PostGame => "PostGame",
        };
        write!(f, "{}", name)
    }
}

/// Connection information for a single player
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Player identifier (0-7)
    pub player_id: u8,
    /// Network address of the peer
    pub peer_address: SocketAddr,
    /// Current connection state
    pub state: ConnectionState,
    /// When the connection was created
    pub created_at: NetworkInstant,
    /// When the state last changed
    pub state_changed_at: NetworkInstant,
    /// When the last packet was received
    pub last_packet_at: NetworkInstant,
    /// Total packets sent to this peer
    pub packets_sent: u64,
    /// Total packets received from this peer
    pub packets_received: u64,
    /// Total bytes sent to this peer
    pub bytes_sent: u64,
    /// Total bytes received from this peer
    pub bytes_received: u64,
    /// Current latency in milliseconds
    pub latency_ms: u32,
}

impl ConnectionInfo {
    /// Create new connection info
    pub fn new(player_id: u8, peer_address: SocketAddr) -> Self {
        let now = NetworkInstant::now();
        Self {
            player_id,
            peer_address,
            state: ConnectionState::PreGame,
            created_at: now,
            state_changed_at: now,
            last_packet_at: now,
            packets_sent: 0,
            packets_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            latency_ms: 0,
        }
    }

    /// Get duration since last packet received
    pub fn time_since_last_packet(&self) -> Duration {
        NetworkInstant::now().duration_since(self.last_packet_at)
    }

    /// Get duration in current state
    pub fn time_in_state(&self) -> Duration {
        NetworkInstant::now().duration_since(self.state_changed_at)
    }

    /// Get total connection duration
    pub fn connection_duration(&self) -> Duration {
        NetworkInstant::now().duration_since(self.created_at)
    }

    /// Check if connection is idle (no packets for timeout duration)
    pub fn is_idle(&self, timeout: Duration) -> bool {
        self.time_since_last_packet() >= timeout
    }

    /// Record packet sent
    pub fn record_packet_sent(&mut self, bytes: u64) {
        self.packets_sent += 1;
        self.bytes_sent += bytes;
    }

    /// Record packet received
    pub fn record_packet_received(&mut self, bytes: u64) {
        self.packets_received += 1;
        self.bytes_received += bytes;
        self.last_packet_at = NetworkInstant::now();
    }

    /// Update latency measurement
    pub fn update_latency(&mut self, latency_ms: u32) {
        self.latency_ms = latency_ms;
    }
}

/// State transition record for history tracking
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// Player who transitioned
    pub player_id: u8,
    /// Previous state
    pub from_state: ConnectionState,
    /// New state
    pub to_state: ConnectionState,
    /// When transition occurred
    pub timestamp: NetworkInstant,
    /// Reason for transition
    pub reason: String,
}

impl StateTransition {
    /// Create new state transition
    pub fn new(
        player_id: u8,
        from_state: ConnectionState,
        to_state: ConnectionState,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            player_id,
            from_state,
            to_state,
            timestamp: NetworkInstant::now(),
            reason: reason.into(),
        }
    }

    /// Get duration since transition
    pub fn age(&self) -> Duration {
        NetworkInstant::now().duration_since(self.timestamp)
    }
}

/// Connection state machine managing all player connections
pub struct ConnectionStateMachine {
    /// Active connections by player ID
    connections: HashMap<u8, ConnectionInfo>,
    /// State transition history (ring buffer)
    state_history: Vec<StateTransition>,
    /// Maximum history entries
    max_history: usize,
    /// Idle timeout for connection cleanup
    idle_timeout: Duration,
}

impl ConnectionStateMachine {
    /// Create new state machine
    pub fn new() -> Self {
        Self::with_config(Duration::from_secs(60), 1000)
    }

    /// Create with custom configuration
    pub fn with_config(idle_timeout: Duration, max_history: usize) -> Self {
        Self {
            connections: HashMap::new(),
            state_history: Vec::with_capacity(max_history.min(10000)),
            max_history: max_history.min(10000),
            idle_timeout,
        }
    }

    /// Add a new connection (starts in PreGame state)
    pub fn add_connection(&mut self, player_id: u8, peer: SocketAddr) -> NetworkResult<()> {
        if self.connections.contains_key(&player_id) {
            return Err(NetworkError::connection(format!(
                "player {} already connected",
                player_id
            )));
        }

        let conn = ConnectionInfo::new(player_id, peer);
        self.connections.insert(player_id, conn);

        info!(
            "Added connection for player {} at {} in PreGame state",
            player_id, peer
        );

        Ok(())
    }

    /// Start game - transition from PreGame to InGame
    pub fn start_game(&mut self, player_id: u8) -> NetworkResult<()> {
        self.transition_state(player_id, ConnectionState::InGame, "game started")
    }

    /// Leave game - transition from InGame to Leaving
    pub fn leave_game(&mut self, player_id: u8) -> NetworkResult<()> {
        self.transition_state(player_id, ConnectionState::Leaving, "player leaving")
    }

    /// Disconnect - transition to Left (terminal state)
    pub fn disconnect(&mut self, player_id: u8) -> NetworkResult<()> {
        self.transition_state(player_id, ConnectionState::Left, "disconnected")
    }

    /// End game - transition from InGame to PostGame
    pub fn end_game(&mut self, player_id: u8) -> NetworkResult<()> {
        self.transition_state(player_id, ConnectionState::PostGame, "game ended")
    }

    /// Get current state for a player
    pub fn get_state(&self, player_id: u8) -> Option<ConnectionState> {
        self.connections.get(&player_id).map(|conn| conn.state)
    }

    /// Get connection info for a player
    pub fn get_connection(&self, player_id: u8) -> Option<&ConnectionInfo> {
        self.connections.get(&player_id)
    }

    /// Get mutable connection info for a player
    pub fn get_connection_mut(&mut self, player_id: u8) -> Option<&mut ConnectionInfo> {
        self.connections.get_mut(&player_id)
    }

    /// Get all active connections
    pub fn connections(&self) -> impl Iterator<Item = &ConnectionInfo> {
        self.connections.values()
    }

    /// Get all player IDs
    pub fn player_ids(&self) -> Vec<u8> {
        self.connections.keys().copied().collect()
    }

    /// Count connections in a specific state
    pub fn count_in_state(&self, state: ConnectionState) -> usize {
        self.connections
            .values()
            .filter(|conn| conn.state == state)
            .count()
    }

    /// Check if all players are in a specific state
    pub fn all_in_state(&self, state: ConnectionState) -> bool {
        !self.connections.is_empty() && self.connections.values().all(|conn| conn.state == state)
    }

    /// Check if any player is in a specific state
    pub fn any_in_state(&self, state: ConnectionState) -> bool {
        self.connections.values().any(|conn| conn.state == state)
    }

    /// Remove idle connections (based on idle timeout)
    pub fn remove_idle_connections(&mut self) -> Vec<u8> {
        let timeout = self.idle_timeout;
        let mut removed = Vec::new();

        self.connections.retain(|&player_id, conn| {
            if conn.is_idle(timeout) && conn.state != ConnectionState::Left {
                warn!(
                    "Removing idle connection for player {} after {:?}",
                    player_id,
                    conn.time_since_last_packet()
                );
                removed.push(player_id);
                false
            } else {
                true
            }
        });

        removed
    }

    /// Remove disconnected connections (Left state)
    pub fn cleanup_disconnected(&mut self) -> Vec<u8> {
        let mut removed = Vec::new();

        self.connections.retain(|&player_id, conn| {
            if conn.state == ConnectionState::Left {
                debug!("Cleaning up disconnected player {}", player_id);
                removed.push(player_id);
                false
            } else {
                true
            }
        });

        removed
    }

    /// Get state transition history
    pub fn history(&self) -> &[StateTransition] {
        &self.state_history
    }

    /// Get recent transitions (last N)
    pub fn recent_transitions(&self, count: usize) -> &[StateTransition] {
        let start = self.state_history.len().saturating_sub(count);
        &self.state_history[start..]
    }

    /// Check if transition is valid according to state machine rules
    pub fn is_valid_transition(&self, from: ConnectionState, to: ConnectionState) -> bool {
        use ConnectionState::*;

        match (from, to) {
            // PreGame can only go to InGame
            (PreGame, InGame) => true,

            // InGame can go to Leaving or PostGame
            (InGame, Leaving) => true,
            (InGame, PostGame) => true,

            // Leaving can only go to Left
            (Leaving, Left) => true,

            // Left is terminal - no transitions allowed
            (Left, _) => false,

            // PostGame can go back to PreGame for rematch
            (PostGame, PreGame) => true,

            // Any state can transition to Left (emergency disconnect)
            (_, Left) => true,

            // Same state is allowed (no-op)
            (a, b) if a == b => true,

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Perform state transition with validation
    fn transition_state(
        &mut self,
        player_id: u8,
        new_state: ConnectionState,
        reason: impl Into<String>,
    ) -> NetworkResult<()> {
        // Get old state first
        let old_state = self
            .connections
            .get(&player_id)
            .ok_or_else(|| NetworkError::connection(format!("player {} not found", player_id)))?
            .state;

        // Validate transition
        if !self.is_valid_transition(old_state, new_state) {
            return Err(NetworkError::invalid_state(format!(
                "invalid transition for player {}: {} -> {}",
                player_id, old_state, new_state
            )));
        }

        // No-op if already in target state
        if old_state == new_state {
            debug!("Player {} already in {} state", player_id, new_state);
            return Ok(());
        }

        // Update state
        let conn = self.connections.get_mut(&player_id).unwrap();
        conn.state = new_state;
        conn.state_changed_at = NetworkInstant::now();

        // Record transition
        let transition = StateTransition::new(player_id, old_state, new_state, reason);
        self.add_to_history(transition.clone());

        info!(
            "Player {} transitioned: {} -> {} (reason: {})",
            player_id, old_state, new_state, transition.reason
        );

        Ok(())
    }

    /// Add transition to history (ring buffer)
    fn add_to_history(&mut self, transition: StateTransition) {
        if self.state_history.len() >= self.max_history {
            self.state_history.remove(0);
        }
        self.state_history.push(transition);
    }

    /// Clear all connections and history
    pub fn clear(&mut self) {
        self.connections.clear();
        self.state_history.clear();
        debug!("Cleared all connections and history");
    }

    /// Get statistics for all connections
    pub fn statistics(&self) -> ConnectionStatistics {
        let mut stats = ConnectionStatistics::default();

        stats.total_connections = self.connections.len();
        stats.pregame_count = self.count_in_state(ConnectionState::PreGame);
        stats.ingame_count = self.count_in_state(ConnectionState::InGame);
        stats.leaving_count = self.count_in_state(ConnectionState::Leaving);
        stats.left_count = self.count_in_state(ConnectionState::Left);
        stats.postgame_count = self.count_in_state(ConnectionState::PostGame);
        stats.total_transitions = self.state_history.len();

        for conn in self.connections.values() {
            stats.total_packets_sent += conn.packets_sent;
            stats.total_packets_received += conn.packets_received;
            stats.total_bytes_sent += conn.bytes_sent;
            stats.total_bytes_received += conn.bytes_received;

            if conn.latency_ms > 0 {
                stats.average_latency_ms += conn.latency_ms as u64;
                stats.latency_sample_count += 1;
            }
        }

        if stats.latency_sample_count > 0 {
            stats.average_latency_ms /= stats.latency_sample_count;
        }

        stats
    }
}

impl Default for ConnectionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for connection state machine
#[derive(Debug, Clone, Default)]
pub struct ConnectionStatistics {
    /// Total number of connections
    pub total_connections: usize,
    /// Connections in PreGame state
    pub pregame_count: usize,
    /// Connections in InGame state
    pub ingame_count: usize,
    /// Connections in Leaving state
    pub leaving_count: usize,
    /// Connections in Left state
    pub left_count: usize,
    /// Connections in PostGame state
    pub postgame_count: usize,
    /// Total state transitions recorded
    pub total_transitions: usize,
    /// Total packets sent across all connections
    pub total_packets_sent: u64,
    /// Total packets received across all connections
    pub total_packets_received: u64,
    /// Total bytes sent across all connections
    pub total_bytes_sent: u64,
    /// Total bytes received across all connections
    pub total_bytes_received: u64,
    /// Average latency in milliseconds
    pub average_latency_ms: u64,
    /// Number of latency samples
    pub latency_sample_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    #[test]
    fn test_connection_state_properties() {
        assert!(!ConnectionState::PreGame.is_terminal());
        assert!(ConnectionState::Left.is_terminal());
        assert!(ConnectionState::InGame.allows_game_activity());
        assert!(!ConnectionState::PreGame.allows_game_activity());
        assert!(ConnectionState::PreGame.is_active());
        assert!(!ConnectionState::Left.is_active());
    }

    #[test]
    fn test_add_connection() {
        let mut sm = ConnectionStateMachine::new();
        let addr = test_addr(8088);

        sm.add_connection(0, addr).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::PreGame));
        assert_eq!(sm.connections.len(), 1);

        // Duplicate should fail
        assert!(sm.add_connection(0, addr).is_err());
    }

    #[test]
    fn test_valid_transition_pregame_to_ingame() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();

        sm.start_game(0).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::InGame));
        assert_eq!(sm.state_history.len(), 1);
    }

    #[test]
    fn test_valid_transition_ingame_to_leaving() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.start_game(0).unwrap();

        sm.leave_game(0).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::Leaving));
        assert_eq!(sm.state_history.len(), 2);
    }

    #[test]
    fn test_valid_transition_leaving_to_left() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.start_game(0).unwrap();
        sm.leave_game(0).unwrap();

        sm.disconnect(0).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::Left));
        assert_eq!(sm.state_history.len(), 3);
    }

    #[test]
    fn test_valid_transition_ingame_to_postgame() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.start_game(0).unwrap();

        sm.end_game(0).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::PostGame));
    }

    #[test]
    fn test_valid_transition_postgame_to_pregame() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.start_game(0).unwrap();
        sm.end_game(0).unwrap();

        // Rematch - go back to PreGame
        let conn = sm.get_connection_mut(0).unwrap();
        let old_state = conn.state;
        conn.state = ConnectionState::PreGame;
        conn.state_changed_at = NetworkInstant::now();

        let transition = StateTransition::new(0, old_state, ConnectionState::PreGame, "rematch");
        sm.add_to_history(transition);

        assert_eq!(sm.get_state(0), Some(ConnectionState::PreGame));
    }

    #[test]
    fn test_invalid_transition_pregame_to_postgame() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();

        // Can't go from PreGame directly to PostGame
        assert!(sm.end_game(0).is_err());
        assert_eq!(sm.get_state(0), Some(ConnectionState::PreGame));
    }

    #[test]
    fn test_invalid_transition_from_left() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.start_game(0).unwrap();
        sm.leave_game(0).unwrap();
        sm.disconnect(0).unwrap();

        // Left is terminal - can't transition anywhere
        assert!(sm.start_game(0).is_err());
        assert_eq!(sm.get_state(0), Some(ConnectionState::Left));
    }

    #[test]
    fn test_emergency_disconnect_from_any_state() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();

        // Can disconnect from PreGame
        sm.disconnect(0).unwrap();
        assert_eq!(sm.get_state(0), Some(ConnectionState::Left));

        // Add another connection in InGame
        sm.add_connection(1, test_addr(8089)).unwrap();
        sm.start_game(1).unwrap();

        // Can disconnect from InGame
        sm.disconnect(1).unwrap();
        assert_eq!(sm.get_state(1), Some(ConnectionState::Left));
    }

    #[test]
    fn test_state_validation() {
        let sm = ConnectionStateMachine::new();

        // Valid transitions
        assert!(sm.is_valid_transition(ConnectionState::PreGame, ConnectionState::InGame));
        assert!(sm.is_valid_transition(ConnectionState::InGame, ConnectionState::Leaving));
        assert!(sm.is_valid_transition(ConnectionState::Leaving, ConnectionState::Left));
        assert!(sm.is_valid_transition(ConnectionState::InGame, ConnectionState::PostGame));
        assert!(sm.is_valid_transition(ConnectionState::PostGame, ConnectionState::PreGame));

        // Emergency disconnect
        assert!(sm.is_valid_transition(ConnectionState::PreGame, ConnectionState::Left));
        assert!(sm.is_valid_transition(ConnectionState::InGame, ConnectionState::Left));

        // Invalid transitions
        assert!(!sm.is_valid_transition(ConnectionState::PreGame, ConnectionState::PostGame));
        assert!(!sm.is_valid_transition(ConnectionState::Left, ConnectionState::InGame));
        assert!(!sm.is_valid_transition(ConnectionState::Leaving, ConnectionState::InGame));
    }

    #[test]
    fn test_count_in_state() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.add_connection(1, test_addr(8089)).unwrap();
        sm.add_connection(2, test_addr(8090)).unwrap();

        assert_eq!(sm.count_in_state(ConnectionState::PreGame), 3);

        sm.start_game(0).unwrap();
        sm.start_game(1).unwrap();

        assert_eq!(sm.count_in_state(ConnectionState::PreGame), 1);
        assert_eq!(sm.count_in_state(ConnectionState::InGame), 2);
    }

    #[test]
    fn test_all_in_state() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.add_connection(1, test_addr(8089)).unwrap();

        assert!(sm.all_in_state(ConnectionState::PreGame));

        sm.start_game(0).unwrap();
        assert!(!sm.all_in_state(ConnectionState::PreGame));
        assert!(!sm.all_in_state(ConnectionState::InGame));

        sm.start_game(1).unwrap();
        assert!(sm.all_in_state(ConnectionState::InGame));
    }

    #[test]
    fn test_connection_metrics() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();

        let conn = sm.get_connection_mut(0).unwrap();
        conn.record_packet_sent(100);
        conn.record_packet_sent(200);
        conn.record_packet_received(150);
        conn.update_latency(50);

        let conn = sm.get_connection(0).unwrap();
        assert_eq!(conn.packets_sent, 2);
        assert_eq!(conn.bytes_sent, 300);
        assert_eq!(conn.packets_received, 1);
        assert_eq!(conn.bytes_received, 150);
        assert_eq!(conn.latency_ms, 50);
    }

    #[test]
    fn test_statistics() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.add_connection(1, test_addr(8089)).unwrap();
        sm.add_connection(2, test_addr(8090)).unwrap();

        sm.start_game(0).unwrap();
        sm.start_game(1).unwrap();

        let conn = sm.get_connection_mut(0).unwrap();
        conn.record_packet_sent(100);
        conn.update_latency(30);

        let conn = sm.get_connection_mut(1).unwrap();
        conn.record_packet_received(200);
        conn.update_latency(50);

        let stats = sm.statistics();
        assert_eq!(stats.total_connections, 3);
        assert_eq!(stats.pregame_count, 1);
        assert_eq!(stats.ingame_count, 2);
        assert_eq!(stats.total_packets_sent, 1);
        assert_eq!(stats.total_packets_received, 1);
        assert_eq!(stats.total_bytes_sent, 100);
        assert_eq!(stats.total_bytes_received, 200);
        assert_eq!(stats.average_latency_ms, 40); // (30 + 50) / 2
    }

    #[test]
    fn test_cleanup_disconnected() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();
        sm.add_connection(1, test_addr(8089)).unwrap();
        sm.add_connection(2, test_addr(8090)).unwrap();

        sm.disconnect(0).unwrap();
        sm.disconnect(2).unwrap();

        let removed = sm.cleanup_disconnected();
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&0));
        assert!(removed.contains(&2));
        assert_eq!(sm.connections.len(), 1);
        assert_eq!(sm.get_state(1), Some(ConnectionState::PreGame));
    }

    #[test]
    fn test_transition_history() {
        let mut sm = ConnectionStateMachine::new();
        sm.add_connection(0, test_addr(8088)).unwrap();

        sm.start_game(0).unwrap();
        sm.end_game(0).unwrap();

        let history = sm.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].from_state, ConnectionState::PreGame);
        assert_eq!(history[0].to_state, ConnectionState::InGame);
        assert_eq!(history[1].from_state, ConnectionState::InGame);
        assert_eq!(history[1].to_state, ConnectionState::PostGame);

        let recent = sm.recent_transitions(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].to_state, ConnectionState::PostGame);
    }

    #[test]
    fn test_multiplayer_scenario() {
        let mut sm = ConnectionStateMachine::new();

        // 4 players join
        for i in 0..4 {
            sm.add_connection(i, test_addr(8088 + i as u16)).unwrap();
        }
        assert!(sm.all_in_state(ConnectionState::PreGame));

        // All start game
        for i in 0..4 {
            sm.start_game(i).unwrap();
        }
        assert!(sm.all_in_state(ConnectionState::InGame));

        // Player 2 disconnects mid-game
        sm.leave_game(2).unwrap();
        sm.disconnect(2).unwrap();
        assert_eq!(sm.count_in_state(ConnectionState::InGame), 3);
        assert_eq!(sm.count_in_state(ConnectionState::Left), 1);

        // Game ends for remaining players
        for i in [0, 1, 3] {
            sm.end_game(i).unwrap();
        }
        assert_eq!(sm.count_in_state(ConnectionState::PostGame), 3);

        // Cleanup
        sm.cleanup_disconnected();
        assert_eq!(sm.connections.len(), 3);
    }

    #[test]
    fn test_history_ring_buffer() {
        let mut sm = ConnectionStateMachine::with_config(Duration::from_secs(60), 5);
        sm.add_connection(0, test_addr(8088)).unwrap();

        // Generate more transitions than max history
        for _ in 0..10 {
            sm.start_game(0).unwrap();
            sm.end_game(0).unwrap();
            // Manually reset to PreGame for next iteration
            let conn = sm.get_connection_mut(0).unwrap();
            conn.state = ConnectionState::PreGame;
        }

        // Should only keep last 5
        assert_eq!(sm.state_history.len(), 5);
    }
}
