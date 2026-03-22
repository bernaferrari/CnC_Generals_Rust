//! Local game discovery and matchmaker for LAN and direct-IP connections.
//!
//! This replaces the GameSpy LAN browser with modern UDP broadcast discovery
//! and direct IP connection support. It is used alongside the cloud-based
//! `MatchmakingService` for players who prefer LAN or direct-IP play.

use crate::commands::NetCommand;
use crate::error::{NetworkError, NetworkResult};
use crate::game_info::GameInfo;
use crate::network_defs::{GENERALS_MAGIC_NUMBER, NETWORK_BASE_PORT_NUMBER};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug, error, info, warn};

/// Broadcast discovery magic for LAN game announcement packets.
/// Sent as the first two bytes of every LAN discovery datagram.
const LAN_DISCOVERY_MAGIC: u16 = 0xC0C0;

/// Interval between LAN broadcast announcements (seconds).
const BROADCAST_INTERVAL_SECS: u64 = 5;

/// Maximum age of a discovered game entry before it is considered stale (seconds).
const GAME_ENTRY_MAX_AGE_SECS: u64 = 30;

/// Maximum number of game entries in the discovery cache.
const MAX_GAME_ENTRIES: usize = 50;

// ---------------------------------------------------------------------------
// Discovered game entry
// ---------------------------------------------------------------------------

/// A game discovered on the local network or added via direct IP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredGame {
    /// Host socket address.
    pub address: SocketAddr,
    /// Host player display name.
    pub host_name: String,
    /// Map currently selected by the host.
    pub map_name: String,
    /// Number of players currently in the game.
    pub player_count: u8,
    /// Maximum number of players allowed.
    pub max_players: u8,
    /// Whether the game has a password.
    pub password_protected: bool,
    /// Arbitrary game-name string chosen by the host.
    pub game_name: String,
    /// Game protocol version (for compatibility filtering).
    pub protocol_version: u32,
    /// Timestamp when this entry was last refreshed.
    pub last_seen: u64, // Unix epoch seconds
    /// Latency estimate in milliseconds (0 = unknown).
    pub latency_ms: u32,
}

impl DiscoveredGame {
    /// Check whether this entry is still fresh.
    pub fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.last_seen) < GAME_ENTRY_MAX_AGE_SECS
    }
}

// ---------------------------------------------------------------------------
// Matchmaker events (async, consumer-friendly)
// ---------------------------------------------------------------------------

/// Events emitted by the `Matchmaker` for the UI / caller to react to.
#[derive(Debug, Clone)]
pub enum MatchmakerEvent {
    /// A new game was discovered on LAN.
    GameDiscovered(DiscoveredGame),
    /// A previously discovered game was updated (player count, map, etc.).
    GameUpdated(DiscoveredGame),
    /// A discovered game is no longer reachable / has expired.
    GameExpired(SocketAddr),
    /// A direct-IP connection attempt succeeded.
    DirectConnectSucceeded { address: SocketAddr },
    /// A direct-IP connection attempt failed.
    DirectConnectFailed { address: SocketAddr, reason: String },
}

// ---------------------------------------------------------------------------
// Game list filter / sort helpers
// ---------------------------------------------------------------------------

/// Criteria for filtering the discovered-game list.
#[derive(Debug, Clone, Default)]
pub struct GameListFilter {
    /// Only show games with available slots.
    pub has_open_slots: bool,
    /// Only show games that are not password-protected.
    pub no_password: bool,
    /// Only show games matching this exact map name (case-insensitive).
    pub map_filter: Option<String>,
    /// Only show games whose host name contains this substring.
    pub host_filter: Option<String>,
    /// Only show games whose name contains this substring.
    pub name_filter: Option<String>,
    /// Minimum protocol version (inclusive).
    pub min_protocol: Option<u32>,
    /// Maximum latency in milliseconds (inclusive).
    pub max_latency_ms: Option<u32>,
}

impl GameListFilter {
    /// Returns `true` if the game passes all filter criteria.
    pub fn matches(&self, game: &DiscoveredGame) -> bool {
        if self.has_open_slots && game.player_count >= game.max_players {
            return false;
        }
        if self.no_password && game.password_protected {
            return false;
        }
        if let Some(ref map) = self.map_filter {
            if !game.map_name.eq_ignore_ascii_case(map) {
                return false;
            }
        }
        if let Some(ref host) = self.host_filter {
            if !game.host_name.to_lowercase().contains(&host.to_lowercase()) {
                return false;
            }
        }
        if let Some(ref name) = self.name_filter {
            if !game.game_name.to_lowercase().contains(&name.to_lowercase()) {
                return false;
            }
        }
        if let Some(min) = self.min_protocol {
            if game.protocol_version < min {
                return false;
            }
        }
        if let Some(max) = self.max_latency_ms {
            if game.latency_ms > max && game.latency_ms != 0 {
                return false;
            }
        }
        true
    }
}

/// Sort order for the game list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameListSort {
    /// Sort by game name (alphabetical).
    Name,
    /// Sort by host name (alphabetical).
    HostName,
    /// Sort by player count (ascending).
    PlayerCount,
    /// Sort by latency (ascending; unknown latency last).
    Latency,
    /// Sort by most recently seen (newest first).
    MostRecent,
}

// ---------------------------------------------------------------------------
// Matchmaker
// ---------------------------------------------------------------------------

/// LAN / direct-IP matchmaker.
///
/// Discovers games on the local network via periodic UDP broadcast and
/// supports connecting directly to a known IP:port.  Runs entirely in
/// user-space -- no GameSpy dependency.
pub struct Matchmaker {
    /// Shared game cache (read by callers, updated by background task).
    games: Arc<RwLock<HashMap<SocketAddr, DiscoveredGame>>>,

    /// Event broadcast channel.
    event_tx: broadcast::Sender<MatchmakerEvent>,

    /// True while the discovery loop is running.
    running: Arc<AtomicBool>,

    /// Handle to the broadcast task.
    broadcast_handle: Mutex<Option<JoinHandle<()>>>,

    /// Handle to the listener task.
    listener_handle: Mutex<Option<JoinHandle<()>>>,

    /// Local protocol version to advertise and filter by.
    protocol_version: u32,

    /// Broadcast port (default 8089, one above the game transport port).
    broadcast_port: u16,

    /// Local player name shown to other discoverers.
    local_player_name: String,

    /// Shutdown signal for background tasks.
    shutdown_tx: broadcast::Sender<()>,
}

impl Matchmaker {
    /// Create a new matchmaker.
    ///
    /// `local_player_name` is the name advertised in LAN discovery packets.
    /// `protocol_version` must match between host and joiner to be compatible.
    pub fn new(local_player_name: String, protocol_version: u32) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            games: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            running: Arc::new(AtomicBool::new(false)),
            broadcast_handle: Mutex::new(None),
            listener_handle: Mutex::new(None),
            protocol_version,
            broadcast_port: NETWORK_BASE_PORT_NUMBER + 1, // 8089
            local_player_name,
            shutdown_tx,
        }
    }

    /// Start LAN discovery (broadcast + listen).
    pub async fn start_discovery(&self) -> NetworkResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.running.store(true, Ordering::SeqCst);

        let shutdown_rx = self.shutdown_tx.subscribe();

        // Spawn listener task.
        let games_clone = self.games.clone();
        let event_tx_clone = self.event_tx.clone();
        let port = self.broadcast_port;
        let running_clone = self.running.clone();
        let shutdown_rx_listen = shutdown_rx.resubscribe();

        let listener = tokio::spawn(async move {
            Self::listener_task(
                games_clone,
                event_tx_clone,
                port,
                running_clone,
                shutdown_rx_listen,
            )
            .await;
        });
        *self.listener_handle.lock().await = Some(listener);

        // Spawn broadcaster task (for hosting -- announces our game).
        let shutdown_rx_bcast = shutdown_rx;
        let running_bcast = self.running.clone();
        let name = self.local_player_name.clone();
        let proto = self.protocol_version;
        let bcast_port = self.broadcast_port;

        let broadcaster = tokio::spawn(async move {
            Self::broadcast_task(name, proto, bcast_port, running_bcast, shutdown_rx_bcast).await;
        });
        *self.broadcast_handle.lock().await = Some(broadcaster);

        info!(
            "Matchmaker discovery started on UDP port {}",
            self.broadcast_port
        );
        Ok(())
    }

    /// Stop LAN discovery and all background tasks.
    pub async fn stop_discovery(&self) -> NetworkResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.running.store(false, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(());

        if let Some(handle) = self.broadcast_handle.lock().await.take() {
            handle.abort();
        }
        if let Some(handle) = self.listener_handle.lock().await.take() {
            handle.abort();
        }

        info!("Matchmaker discovery stopped");
        Ok(())
    }

    /// Subscribe to matchmaker events.
    pub fn subscribe(&self) -> broadcast::Receiver<MatchmakerEvent> {
        self.event_tx.subscribe()
    }

    /// Get a snapshot of all currently known games.
    pub async fn game_list(&self) -> Vec<DiscoveredGame> {
        let games = self.games.read().await;
        games.values().cloned().filter(|g| g.is_fresh()).collect()
    }

    /// Get filtered + sorted game list.
    pub async fn filtered_game_list(
        &self,
        filter: &GameListFilter,
        sort: GameListSort,
    ) -> Vec<DiscoveredGame> {
        let mut games: Vec<DiscoveredGame> = self
            .games
            .read()
            .await
            .values()
            .cloned()
            .filter(|g| g.is_fresh() && filter.matches(g))
            .collect();

        match sort {
            GameListSort::Name => {
                games.sort_by(|a, b| a.game_name.to_lowercase().cmp(&b.game_name.to_lowercase()))
            }
            GameListSort::HostName => {
                games.sort_by(|a, b| a.host_name.to_lowercase().cmp(&b.host_name.to_lowercase()))
            }
            GameListSort::PlayerCount => games.sort_by_key(|g| g.player_count),
            GameListSort::Latency => games.sort_by_key(|g| g.latency_ms),
            GameListSort::MostRecent => games.sort_by(|a, b| b.last_seen.cmp(&a.last_seen)),
        }
        games
    }

    /// Manually add / refresh a game entry (used when receiving game info
    /// through the transport layer from a known peer).
    pub async fn add_game(&self, game: DiscoveredGame) {
        let is_new = !self.games.read().await.contains_key(&game.address);
        self.games.write().await.insert(game.address, game.clone());

        let _ = self.event_tx.send(if is_new {
            MatchmakerEvent::GameDiscovered(game)
        } else {
            MatchmakerEvent::GameUpdated(game)
        });
    }

    /// Remove a game entry (e.g. host disconnected).
    pub async fn remove_game(&self, addr: SocketAddr) {
        if self.games.write().await.remove(&addr).is_some() {
            let _ = self.event_tx.send(MatchmakerEvent::GameExpired(addr));
        }
    }

    /// Attempt a direct-IP connection.
    ///
    /// Sends a ping to the given address and waits up to `timeout` for a
    /// response.  Returns `Ok(())` on success.
    pub async fn direct_connect(&self, addr: SocketAddr, timeout: Duration) -> NetworkResult<()> {
        info!("Attempting direct connection to {}", addr);

        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| NetworkError::transport(format!("Failed to bind UDP socket: {}", e)))?;

        // Send a discovery ping.
        let packet = Self::build_discovery_packet(
            &self.local_player_name,
            self.protocol_version,
            true, // is_ping
        );
        socket
            .send_to(&packet, addr)
            .await
            .map_err(|e| NetworkError::transport(format!("Failed to send ping: {}", e)))?;

        // Wait for response.
        let mut buf = [0u8; 512];
        match time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _src))) => {
                if len >= 2 {
                    let magic = u16::from_le_bytes([buf[0], buf[1]]);
                    if magic == LAN_DISCOVERY_MAGIC {
                        let _ = self
                            .event_tx
                            .send(MatchmakerEvent::DirectConnectSucceeded { address: addr });
                        info!("Direct connect to {} succeeded", addr);
                        return Ok(());
                    }
                }
                let _ = self.event_tx.send(MatchmakerEvent::DirectConnectFailed {
                    address: addr,
                    reason: "Invalid response packet".to_string(),
                });
                Err(NetworkError::transport("Invalid response from host"))
            }
            Ok(Err(e)) => {
                let reason = format!("Receive error: {}", e);
                let _ = self.event_tx.send(MatchmakerEvent::DirectConnectFailed {
                    address: addr,
                    reason: reason.clone(),
                });
                Err(NetworkError::transport(reason))
            }
            Err(_) => {
                let reason = format!("Connection timed out after {:?}", timeout);
                let _ = self.event_tx.send(MatchmakerEvent::DirectConnectFailed {
                    address: addr,
                    reason: reason.clone(),
                });
                Err(NetworkError::transport(reason))
            }
        }
    }

    /// Announce a hosted game so that LAN peers can discover it.
    ///
    /// Call this periodically (or once; the broadcast task will repeat it).
    pub async fn announce_hosted_game(
        &self,
        game_info: &GameInfo,
        game_name: &str,
        password_protected: bool,
    ) -> NetworkResult<()> {
        let game = DiscoveredGame {
            address: SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::UNSPECIFIED,
                NETWORK_BASE_PORT_NUMBER,
            )),
            host_name: self.local_player_name.clone(),
            map_name: game_info.get_map().to_string(),
            player_count: game_info.get_num_players() as u8,
            max_players: crate::config::MAX_SLOTS as u8,
            password_protected,
            game_name: game_name.to_string(),
            protocol_version: self.protocol_version,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            latency_ms: 0,
        };

        // Store locally so listeners see it.
        self.games.write().await.insert(game.address, game);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn build_discovery_packet(name: &str, protocol_version: u32, is_ping: bool) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        // Magic header
        buf.extend_from_slice(&LAN_DISCOVERY_MAGIC.to_le_bytes());
        // Flags: bit 0 = ping request (1) vs announcement (0)
        let flags: u8 = if is_ping { 1 } else { 0 };
        buf.push(flags);
        // Protocol version (4 bytes LE)
        buf.extend_from_slice(&protocol_version.to_le_bytes());
        // Player name length (1 byte) + name bytes
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(255) as u8;
        buf.push(name_len);
        buf.extend_from_slice(&name_bytes[..name_len as usize]);
        buf
    }

    fn parse_discovery_packet(data: &[u8], src_addr: SocketAddr) -> Option<(String, u32, bool)> {
        if data.len() < 7 {
            return None;
        }
        let magic = u16::from_le_bytes([data[0], data[1]]);
        if magic != LAN_DISCOVERY_MAGIC {
            return None;
        }
        let is_ping = (data[2] & 1) != 0;
        let protocol_version = u32::from_le_bytes([data[3], data[4], data[5], data[6]]);
        let name_len = *data.get(7)? as usize;
        if data.len() < 8 + name_len {
            return None;
        }
        let name = String::from_utf8_lossy(&data[8..8 + name_len]).to_string();
        Some((name, protocol_version, is_ping))
    }

    /// Background listener: receives broadcast packets and updates game list.
    async fn listener_task(
        games: Arc<RwLock<HashMap<SocketAddr, DiscoveredGame>>>,
        event_tx: broadcast::Sender<MatchmakerEvent>,
        port: u16,
        running: Arc<AtomicBool>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
        let socket = match UdpSocket::bind(bind_addr) {
            Ok(s) => {
                if s.set_broadcast(true).is_err() {
                    warn!("Failed to enable broadcast on discovery socket");
                }
                s
            }
            Err(e) => {
                error!("Failed to bind discovery socket on port {}: {}", port, e);
                return;
            }
        };

        let tokio_socket = match tokio::net::UdpSocket::from_std(socket) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to convert discovery socket: {}", e);
                return;
            }
        };

        let mut buf = [0u8; 512];
        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => break,
                result = tokio_socket.recv_from(&mut buf) => {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    match result {
                        Ok((len, src)) => {
                            if let Some((name, proto, is_ping)) =
                                Self::parse_discovery_packet(&buf[..len], src)
                            {
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let is_new = !games.read().await.contains_key(&src);
                                let mut games_lock = games.write().await;
                                let entry = games_lock.entry(src).or_insert_with(|| {
                                    DiscoveredGame {
                                        address: src,
                                        host_name: name.clone(),
                                        map_name: String::new(),
                                        player_count: 0,
                                        max_players: 8,
                                        password_protected: false,
                                        game_name: String::new(),
                                        protocol_version: proto,
                                        last_seen: now,
                                        latency_ms: 0,
                                    }
                                });
                                entry.host_name = name;
                                entry.protocol_version = proto;
                                entry.last_seen = now;
                                let game = entry.clone();
                                drop(games_lock);

                                let _ = event_tx.send(if is_new {
                                    MatchmakerEvent::GameDiscovered(game)
                                } else {
                                    MatchmakerEvent::GameUpdated(game)
                                });

                                // Reply to ping requests so the requester knows we exist.
                                if is_ping {
                                    // Response is handled in broadcast_task via periodic sends.
                                    debug!("Received discovery ping from {}", src);
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Discovery recv error: {}", e);
                        }
                    }
                }
            }
        }
        debug!("Discovery listener task stopped");
    }

    /// Background broadcaster: periodically sends LAN announcement.
    async fn broadcast_task(
        name: String,
        protocol_version: u32,
        port: u16,
        running: Arc<AtomicBool>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let socket = match UdpSocket::bind(bind_addr) {
            Ok(s) => {
                if s.set_broadcast(true).is_err() {
                    warn!("Failed to enable broadcast on sender socket");
                }
                s
            }
            Err(e) => {
                error!("Failed to bind broadcast sender: {}", e);
                return;
            }
        };

        let tokio_socket = match tokio::net::UdpSocket::from_std(socket) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to convert broadcast socket: {}", e);
                return;
            }
        };

        let broadcast_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::BROADCAST, port).into();
        let packet = Self::build_discovery_packet(&name, protocol_version, false);
        let mut interval = time::interval(Duration::from_secs(BROADCAST_INTERVAL_SECS));

        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(e) = tokio_socket.send_to(&packet, broadcast_addr).await {
                        debug!("Broadcast send error: {}", e);
                    }
                }
            }
        }
        debug!("Discovery broadcast task stopped");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_game_freshness() {
        let mut game = DiscoveredGame {
            address: "127.0.0.1:8088".parse().unwrap(),
            host_name: "Host".to_string(),
            map_name: "Map".to_string(),
            player_count: 1,
            max_players: 8,
            password_protected: false,
            game_name: "Test".to_string(),
            protocol_version: 1,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            latency_ms: 0,
        };
        assert!(game.is_fresh());

        game.last_seen = 0;
        assert!(!game.is_fresh());
    }

    #[test]
    fn test_game_list_filter() {
        let mut game = DiscoveredGame {
            address: "127.0.0.1:8088".parse().unwrap(),
            host_name: "Alice".to_string(),
            map_name: "DesertFury".to_string(),
            player_count: 2,
            max_players: 8,
            password_protected: false,
            game_name: "Fun Game".to_string(),
            protocol_version: 2,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            latency_ms: 50,
        };

        let filter = GameListFilter::default();
        assert!(filter.matches(&game));

        let filter_full = GameListFilter {
            has_open_slots: false, // would require player_count < max_players, which is true
            no_password: true,
            ..Default::default()
        };
        assert!(filter_full.matches(&game));

        let filter_no_password = GameListFilter {
            no_password: true,
            ..Default::default()
        };
        assert!(filter_no_password.matches(&game));

        game.password_protected = true;
        assert!(!filter_no_password.matches(&game));

        let filter_map = GameListFilter {
            map_filter: Some("desertfury".to_string()),
            ..Default::default()
        };
        assert!(filter_map.matches(&game));

        let filter_map_no = GameListFilter {
            map_filter: Some("other_map".to_string()),
            ..Default::default()
        };
        assert!(!filter_map_no.matches(&game));

        let filter_proto = GameListFilter {
            min_protocol: Some(3),
            ..Default::default()
        };
        assert!(!filter_proto.matches(&game));
    }

    #[test]
    fn test_discovery_packet_roundtrip() {
        let packet = Matchmaker::build_discovery_packet("TestPlayer", 2, true);
        let parsed = Matchmaker::parse_discovery_packet(&packet, "0.0.0.0:0".parse().unwrap());
        let (name, proto, is_ping) = parsed.unwrap();
        assert_eq!(name, "TestPlayer");
        assert_eq!(proto, 2);
        assert!(is_ping);
    }

    #[test]
    fn test_discovery_packet_invalid_magic() {
        let data = vec![0xFF, 0xFF, 0, 0, 0, 0, 0];
        assert!(Matchmaker::parse_discovery_packet(&data, "0.0.0.0:0".parse().unwrap()).is_none());
    }

    #[test]
    fn test_discovery_packet_too_short() {
        let data = vec![0xC0, 0x0C, 0];
        assert!(Matchmaker::parse_discovery_packet(&data, "0.0.0.0:0".parse().unwrap()).is_none());
    }

    #[tokio::test]
    async fn test_matchmaker_creation() {
        let mm = Matchmaker::new("Player1".to_string(), 1);
        assert_eq!(mm.local_player_name, "Player1");
        assert_eq!(mm.protocol_version, 1);
        assert!(!mm.running.load(Ordering::SeqCst));
        assert!(mm.game_list().await.is_empty());
    }
}
