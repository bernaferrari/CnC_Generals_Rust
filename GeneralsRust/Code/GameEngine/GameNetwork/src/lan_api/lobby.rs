//! LAN lobby management for game sessions
//!
//! This module handles lobby coordination, player management, and game state
//! transitions for LAN games. It provides the core functionality for hosting
//! and joining games.

use crate::connection::ConnectionManager;
use crate::error::{NetworkError, NetworkResult};
use crate::lan_api::crypto::LanCrypto;
use crate::lan_api::discovery::GameAnnouncement;
use crate::lan_api::game_info::GameState;
use crate::lan_api::messages::PlayerInfo;
use crate::lan_api::{
    ChatMessage, ChatType, GameDiscovery, GameOptions, LanBridgeEvent, LanConfig, LanEventSender,
    LanGameInfo, LanMessage, LanPlayer,
};
use crate::security::SecurityManager;
use crate::time::NetworkInstant;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket as AsyncUdpSocket;
use tokio::sync::{Notify, RwLock};
use tokio::time::interval;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

/// Lobby state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyState {
    /// Not in any lobby
    None,
    /// In the main lobby (list of games)
    MainLobby,
    /// In a game lobby (game setup)
    GameLobby,
    /// Game is starting
    Starting,
    /// Game is in progress
    InGame,
}

impl Default for LobbyState {
    fn default() -> Self {
        Self::None
    }
}

/// Lobby events that can occur
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyEvent {
    /// Player joined the lobby/game
    PlayerJoined(LanPlayer),
    /// Player left the lobby/game
    PlayerLeft(IpAddr, String), // ip, name
    /// Game was created
    GameCreated(LanGameInfo),
    /// Game was joined
    GameJoined(LanGameInfo, u8), // game, slot
    /// Game options were updated
    GameOptionsUpdated(GameOptions),
    /// Player accepted game settings
    PlayerAccepted(IpAddr, bool),
    /// Player map status updated
    PlayerMapStatus(IpAddr, bool),
    /// Player changed name
    NameChange(IpAddr, String),
    /// Game start countdown
    GameStartTimer(u32), // seconds
    /// Game is starting
    GameStarting,
    /// Host left the game
    HostLeft,
    /// Local player inactive status changed
    PlayerInactive(IpAddr),
    /// Error occurred
    Error(String),
}

/// Join request information
#[derive(Debug, Clone)]
struct JoinRequest {
    game: LanGameInfo,
    target_ip: Option<IpAddr>,
    requested_at: NetworkInstant,
}

#[derive(Debug, Clone)]
struct DirectConnectState {
    target_ip: IpAddr,
    requested_at: NetworkInstant,
}

/// LAN lobby management
pub struct LanLobby {
    /// Static configuration derived from the LAN API settings.
    config: LanConfig,
    /// Discovery service used to announce and discover games.
    discovery: Arc<GameDiscovery>,
    /// Encryption helper for securing LAN datagrams.
    crypto: LanCrypto,
    /// Current lobby state
    state: Arc<RwLock<LobbyState>>,
    /// UDP socket for communication
    socket: Arc<RwLock<Option<Arc<AsyncUdpSocket>>>>,
    /// Local endpoint (ip+port) used for announcements
    local_endpoint: Arc<RwLock<Option<SocketAddr>>>,
    /// Preferred local IP set before sockets are initialised
    preferred_local_ip: Arc<RwLock<Option<IpAddr>>>,
    /// Publicly reachable endpoint discovered via NAT traversal
    public_endpoint: Arc<RwLock<Option<SocketAddr>>>,
    /// Current game we're in (if any)
    current_game: Arc<RwLock<Option<LanGameInfo>>>,
    /// Whether we're hosting
    is_hosting: Arc<RwLock<bool>>,
    /// Game identifier currently advertised via discovery (if any)
    hosted_game_id: Arc<RwLock<Option<Uuid>>>,
    /// Local player information
    local_player: Arc<RwLock<Option<LanPlayer>>>,
    /// Bridge back into the high-level [`LanApi`].
    bridge_tx: LanEventSender,
    /// Background tasks
    tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    /// Whether the lobby is active
    is_active: Arc<RwLock<bool>>,
    /// Current join request (if any)
    join_request: Arc<RwLock<Option<JoinRequest>>>,
    /// Game start timer
    game_start_timer: Arc<RwLock<Option<NetworkInstant>>>,
    /// Pending direct connect handshake
    pending_direct_connect: Arc<RwLock<Option<DirectConnectState>>>,
    /// Shutdown notifier for background tasks
    shutdown_notify: Arc<Notify>,
    /// Whether real network IO should be used
    networking_enabled: bool,
}

impl LanLobby {
    fn build(
        config: LanConfig,
        discovery: Arc<GameDiscovery>,
        bridge_tx: LanEventSender,
        networking_enabled: bool,
        crypto: LanCrypto,
    ) -> Self {
        Self {
            config,
            discovery,
            crypto,
            state: Arc::new(RwLock::new(LobbyState::default())),
            socket: Arc::new(RwLock::new(None)),
            local_endpoint: Arc::new(RwLock::new(None)),
            preferred_local_ip: Arc::new(RwLock::new(None)),
            public_endpoint: Arc::new(RwLock::new(None)),
            current_game: Arc::new(RwLock::new(None)),
            is_hosting: Arc::new(RwLock::new(false)),
            hosted_game_id: Arc::new(RwLock::new(None)),
            local_player: Arc::new(RwLock::new(None)),
            bridge_tx,
            tasks: Arc::new(RwLock::new(Vec::new())),
            is_active: Arc::new(RwLock::new(false)),
            join_request: Arc::new(RwLock::new(None)),
            game_start_timer: Arc::new(RwLock::new(None)),
            pending_direct_connect: Arc::new(RwLock::new(None)),
            shutdown_notify: Arc::new(Notify::new()),
            networking_enabled,
        }
    }

    /// Create a new LAN lobby
    /// Create a new LAN lobby
    pub async fn new(
        config: LanConfig,
        discovery: Arc<GameDiscovery>,
        bridge_tx: LanEventSender,
    ) -> NetworkResult<Self> {
        Ok(Self::build(
            config,
            discovery,
            bridge_tx,
            true,
            LanCrypto::default(),
        ))
    }

    /// Create a lobby with explicit security and connection context.
    pub async fn with_dependencies(
        config: LanConfig,
        discovery: Arc<GameDiscovery>,
        bridge_tx: LanEventSender,
        security: Option<Arc<SecurityManager>>,
        connections: Option<Arc<RwLock<ConnectionManager>>>,
    ) -> NetworkResult<Self> {
        Ok(Self::build(
            config,
            discovery,
            bridge_tx,
            true,
            LanCrypto::new(security, connections),
        ))
    }

    #[cfg(test)]
    pub(super) async fn new_test(
        config: LanConfig,
        discovery: Arc<GameDiscovery>,
        bridge_tx: LanEventSender,
    ) -> NetworkResult<Self> {
        Ok(Self::build(
            config,
            discovery,
            bridge_tx,
            false,
            LanCrypto::default(),
        ))
    }

    /// Initialize the lobby
    pub async fn init(&mut self) -> NetworkResult<()> {
        info!("Initializing LAN lobby");

        *self.is_active.write().await = true;
        *self.state.write().await = LobbyState::MainLobby;
        self.shutdown_notify = Arc::new(Notify::new());

        if self.networking_enabled {
            self.init_socket().await?;
        } else {
            // Ensure we have a usable endpoint even when skipping IO.
            if self.local_endpoint.read().await.is_none() {
                if let Some(ip) = *self.preferred_local_ip.read().await {
                    let addr = SocketAddr::new(ip, self.config.base_port);
                    *self.local_endpoint.write().await = Some(addr);
                }
            }
        }

        // Start background tasks
        self.start_background_tasks().await;

        info!("LAN lobby initialized successfully");
        Ok(())
    }

    /// Initialize UDP socket
    async fn init_socket(&self) -> NetworkResult<()> {
        // Prefer the configured base port when available to match discovery semantics.
        let socket =
            match AsyncUdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], self.config.base_port)))
                .await
            {
                Ok(sock) => Arc::new(sock),
                Err(primary_err) => Arc::new(AsyncUdpSocket::bind("0.0.0.0:0").await.map_err(
                    |fallback_err| {
                        NetworkError::transport(format!(
                            "Failed to bind UDP socket ({}); fallback also failed: {}",
                            primary_err, fallback_err
                        ))
                    },
                )?),
            };
        socket
            .set_broadcast(true)
            .map_err(|e| NetworkError::transport(format!("Failed to enable broadcast: {}", e)))?;
        let addr = socket.local_addr().map_err(|e| {
            NetworkError::transport(format!("Failed to query socket address: {}", e))
        })?;

        let port = addr.port();
        let preferred_ip = {
            let guard = self.preferred_local_ip.read().await;
            guard.as_ref().copied().unwrap_or(addr.ip())
        };
        let endpoint = SocketAddr::new(preferred_ip, port);

        *self.socket.write().await = Some(socket);
        *self.local_endpoint.write().await = Some(endpoint);
        debug!("UDP socket initialised on {}", endpoint);
        Ok(())
    }

    /// Update the local announcement endpoint with the resolved external IP.
    pub async fn set_local_ip(&self, ip: IpAddr) {
        {
            let mut guard = self.preferred_local_ip.write().await;
            *guard = Some(ip);
        }

        let port = {
            let guard = self.local_endpoint.read().await;
            guard
                .as_ref()
                .map(|addr| addr.port())
                .unwrap_or(self.config.base_port)
        };
        let endpoint = SocketAddr::new(ip, port);
        *self.local_endpoint.write().await = Some(endpoint);

        let hosted_game = {
            let mut guard = self.current_game.write().await;
            guard.as_mut().map(|game| {
                game.host_ip = ip;
                game.port = port;
                for slot in &mut game.slots {
                    if let Some(player) = slot.player.as_mut() {
                        if player.is_host() {
                            player.ip = ip;
                            player.port = port;
                            break;
                        }
                    }
                }
                game.game_id
            })
        };

        {
            let mut guard = self.local_player.write().await;
            if let Some(player) = guard.as_mut() {
                player.ip = ip;
                player.port = port;
            }
        }

        if self.networking_enabled {
            self.discovery.set_local_ip(ip).await;

            if let Some(game_id) = hosted_game {
                if let Err(err) = self.discovery.refresh_local(game_id).await {
                    warn!(
                        "Failed to refresh announcement after local IP update: {}",
                        err
                    );
                }
            }
        }
    }

    /// Update the public endpoint used for advertisements and direct connect.
    pub async fn set_public_endpoint(&mut self, endpoint: Option<SocketAddr>) {
        *self.public_endpoint.write().await = endpoint;

        if !*self.is_hosting.read().await {
            return;
        }

        let snapshot = {
            let mut guard = self.current_game.write().await;
            if let Some(game) = guard.as_mut() {
                game.public_host = endpoint.map(|addr| addr.ip());
                game.public_port = endpoint.map(|addr| addr.port());
                Some(game.clone())
            } else {
                None
            }
        };

        if self.networking_enabled {
            if let Some(game) = snapshot {
                let announcement = self.build_announcement(&game);
                if let Err(err) = self.discovery.publish_local(announcement).await {
                    warn!(
                        "Failed to update discovery announcement after public endpoint change: {}",
                        err
                    );
                }
                if let Err(err) = self.discovery.refresh_local(game.game_id).await {
                    warn!(
                        "Failed to refresh discovery announcement after public endpoint change: {}",
                        err
                    );
                }
            }
        }
    }

    pub(super) async fn player_info(&self) -> Option<PlayerInfo> {
        let endpoint = *self.local_endpoint.read().await;
        let endpoint = endpoint?;

        let mut info = PlayerInfo::new(
            self.config.player_name.clone(),
            endpoint.ip(),
            endpoint.port(),
        );
        info.login_name = self.config.login_name.clone();
        info.host_name = self.config.host_name.clone();
        Some(info)
    }

    pub(super) async fn send_to(
        &self,
        message: &LanMessage,
        target: SocketAddr,
    ) -> NetworkResult<()> {
        if !self.networking_enabled {
            return Ok(());
        }

        let socket = {
            let guard = self.socket.read().await;
            guard.clone()
        };
        let socket = socket.ok_or_else(|| {
            NetworkError::transport("LAN lobby socket not initialised".to_string())
        })?;
        let bytes = message.to_bytes().map_err(|err| {
            NetworkError::transport(format!("Failed to serialize LAN message: {}", err))
        })?;
        let payload = self.crypto.encode(&bytes, target).await;
        socket.send_to(&payload, target).await.map_err(|err| {
            NetworkError::transport(format!("Failed to send LAN message: {}", err))
        })?;
        Ok(())
    }

    async fn broadcast_message(&self, message: &LanMessage) -> NetworkResult<()> {
        let target = SocketAddr::from((self.config.broadcast_addr, self.config.base_port));
        self.send_to(message, target).await
    }

    async fn host_endpoint(&self) -> Option<SocketAddr> {
        let guard = self.current_game.read().await;
        guard
            .as_ref()
            .map(|game| SocketAddr::new(game.host_ip, game.port))
    }

    async fn remote_participants(&self) -> Vec<SocketAddr> {
        let guard = self.current_game.read().await;
        guard
            .as_ref()
            .map(|game| {
                game.slots
                    .iter()
                    .filter_map(|slot| slot.player.as_ref())
                    .filter_map(|player| {
                        if player.is_host() {
                            None
                        } else {
                            Some(SocketAddr::new(player.ip, player.port))
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn send_to_host(&self, message: &LanMessage) -> NetworkResult<()> {
        let addr = self.host_endpoint().await.ok_or_else(|| {
            NetworkError::invalid_command("Host endpoint unavailable".to_string())
        })?;
        self.send_to(message, addr).await
    }

    async fn send_to_remote_players(&self, message: &LanMessage) -> NetworkResult<()> {
        for addr in self.remote_participants().await {
            self.send_to(message, addr).await?;
        }
        Ok(())
    }

    async fn send_to_game_participants(&self, message: &LanMessage) -> NetworkResult<()> {
        if *self.is_hosting.read().await {
            self.send_to_remote_players(message).await
        } else {
            self.send_to_host(message).await
        }
    }

    async fn emit_lobby_event(&self, event: LobbyEvent) {
        let _ = self.bridge_tx.send(LanBridgeEvent::LobbyEvent(event));
    }

    async fn local_player_ip(&self) -> Option<IpAddr> {
        self.local_player
            .read()
            .await
            .as_ref()
            .map(|player| player.ip)
    }

    pub(super) async fn update_player_acceptance(&self, ip: IpAddr, accepted: bool) {
        if let Some(ref mut game) = *self.current_game.write().await {
            if game.set_player_accepted(ip, accepted) {
                game.update_player_last_heard(ip);
            }
        }
    }

    pub(super) async fn update_player_map_status(&self, ip: IpAddr, has_map: bool) {
        if let Some(ref mut game) = *self.current_game.write().await {
            if game.set_player_has_map(ip, has_map) {
                game.update_player_last_heard(ip);
            }
        }
    }

    pub(super) async fn apply_remote_game_options(&self, options: &GameOptions, is_public: bool) {
        if let Some(ref mut game) = *self.current_game.write().await {
            game.options = options.clone();
            game.is_public = is_public;
            for slot in &mut game.slots {
                if let Some(player) = slot.player.as_ref() {
                    if !player.is_host() {
                        slot.has_accepted = false;
                    }
                }
            }
        }
    }

    pub(super) async fn update_player_name(&self, ip: IpAddr, new_name: &str) {
        if let Some(ref mut game) = *self.current_game.write().await {
            if let Some(slot) = game
                .slots
                .iter_mut()
                .find(|slot| slot.player.as_ref().map(|p| p.ip) == Some(ip))
            {
                if let Some(player) = slot.player.as_mut() {
                    player.name = new_name.to_string();
                }
            }
        }
    }

    pub(super) async fn set_game_start_timer_internal(&self, seconds: Option<u32>) {
        match seconds {
            Some(delay) if delay == 0 => {
                *self.game_start_timer.write().await = None;
                *self.state.write().await = LobbyState::GameLobby;
                self.emit_lobby_event(LobbyEvent::GameStartTimer(0)).await;
            }
            Some(delay) => {
                let timer_end = NetworkInstant::now() + Duration::from_secs(delay as u64);
                *self.game_start_timer.write().await = Some(timer_end);
                *self.state.write().await = LobbyState::Starting;
                self.emit_lobby_event(LobbyEvent::GameStartTimer(delay))
                    .await;
            }
            None => {
                *self.game_start_timer.write().await = None;
                *self.state.write().await = LobbyState::Starting;
                self.emit_lobby_event(LobbyEvent::GameStarting).await;
            }
        }
    }

    fn build_announcement(&self, game: &LanGameInfo) -> GameAnnouncement {
        GameAnnouncement {
            game_id: game.game_id,
            name: game.name.clone(),
            host: game.host_ip,
            port: game.port,
            player_count: game.player_count,
            max_players: game.max_players,
            has_password: game.has_password,
            is_public: game.is_public,
            is_direct_connect: game.is_direct_connect,
            version_hash: game.version_hash,
            map_crc: game.map_crc,
            options: game.options.clone(),
            public_host: game.public_host,
            public_port: game.public_port,
        }
    }

    pub async fn current_game_snapshot(&self) -> Option<LanGameInfo> {
        self.current_game.read().await.clone()
    }

    pub async fn add_player_to_current_game(
        &self,
        mut player: LanPlayer,
    ) -> NetworkResult<(LanGameInfo, u8)> {
        player.joined_at = Utc::now();
        let mut guard = self.current_game.write().await;
        let game = guard.as_mut().ok_or_else(|| {
            NetworkError::invalid_command("No active game to add players".to_string())
        })?;

        let slot = game
            .add_player(player)
            .map_err(|err| NetworkError::invalid_command(err))?;
        let snapshot = game.clone();
        drop(guard);

        if self.networking_enabled {
            let announcement = self.build_announcement(&snapshot);
            if let Err(err) = self.discovery.publish_local(announcement).await {
                warn!("Failed to republish discovery announcement: {}", err);
            }
            if let Err(err) = self.discovery.refresh_local(snapshot.game_id).await {
                warn!("Failed to refresh discovery announcement: {}", err);
            }
        }

        if let Some(player) = snapshot.slots[slot as usize].player.clone() {
            self.emit_lobby_event(LobbyEvent::PlayerJoined(player))
                .await;
        }

        Ok((snapshot, slot))
    }

    pub async fn remove_player_from_current_game(&self, ip: IpAddr) -> Option<LanGameInfo> {
        let mut guard = self.current_game.write().await;
        let game = guard.as_mut()?;
        let player_name = game
            .get_player_by_ip(ip)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        if !game.remove_player(ip) {
            return None;
        }
        let snapshot = game.clone();
        drop(guard);

        if self.networking_enabled {
            let announcement = self.build_announcement(&snapshot);
            if let Err(err) = self.discovery.publish_local(announcement).await {
                warn!("Failed to republish discovery announcement: {}", err);
            }
            if let Err(err) = self.discovery.refresh_local(snapshot.game_id).await {
                warn!("Failed to refresh discovery announcement: {}", err);
            }
        }

        self.emit_lobby_event(LobbyEvent::PlayerLeft(ip, player_name))
            .await;
        Some(snapshot)
    }

    pub async fn request_locations(&self) -> NetworkResult<()> {
        if let Some(info) = self.player_info().await {
            let message = LanMessage::request_locations(info);
            self.broadcast_message(&message).await?;
        }
        Ok(())
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Start timer task
        self.start_timer_task().await;

        if self.networking_enabled {
            self.start_socket_receiver().await;
        }
    }

    /// Start timer task for countdowns
    async fn start_timer_task(&self) {
        let game_start_timer = Arc::clone(&self.game_start_timer);
        let bridge_tx = self.bridge_tx.clone();
        let is_active = Arc::clone(&self.is_active);
        let shutdown = Arc::clone(&self.shutdown_notify);

        let handle = tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_millis(100));

            loop {
                tokio::select! {
                    _ = shutdown.notified() => {
                        break;
                    }
                    _ = check_interval.tick() => {
                        if !*is_active.read().await {
                            continue;
                        }

                        if let Some(timer_end) = *game_start_timer.read().await {
                            let now = NetworkInstant::now();
                            if now >= timer_end {
                                let _ = bridge_tx
                                    .send(LanBridgeEvent::LobbyEvent(LobbyEvent::GameStarting));
                                *game_start_timer.write().await = None;
                            } else {
                                let remaining = timer_end
                                    .duration_since(now)
                                    .as_secs()
                                    .clamp(0, u64::from(u32::MAX)) as u32;
                                let _ = bridge_tx.send(LanBridgeEvent::LobbyEvent(
                                    LobbyEvent::GameStartTimer(remaining),
                                ));
                            }
                        }
                    }
                }
            }

            debug!("Timer task stopped");
        });

        self.tasks.write().await.push(handle);
    }

    /// Start socket receiver task
    async fn start_socket_receiver(&self) {
        const MAX_DATAGRAM: usize = 1400;

        let socket = Arc::clone(&self.socket);
        let bridge_tx = self.bridge_tx.clone();
        let is_active = Arc::clone(&self.is_active);
        let shutdown = Arc::clone(&self.shutdown_notify);
        let crypto = self.crypto.clone();

        let handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; MAX_DATAGRAM];

            loop {
                let socket_instance = {
                    let guard = socket.read().await;
                    guard.clone()
                };

                let Some(sock) = socket_instance else {
                    if tokio::select! {
                        _ = shutdown.notified() => true,
                        _ = tokio::time::sleep(Duration::from_millis(100)) => false,
                    } {
                        break;
                    } else {
                        continue;
                    }
                };

                let recv_result = match tokio::select! {
                    _ = shutdown.notified() => None,
                    res = sock.recv_from(&mut buffer) => Some(res),
                } {
                    Some(res) => res,
                    None => break,
                };

                if !*is_active.read().await {
                    continue;
                }

                match recv_result {
                    Ok((len, sender)) => {
                        trace!("Received lobby message from {} ({} bytes)", sender, len);
                        if len == 0 {
                            continue;
                        }

                        match crypto.decode(&buffer[..len], sender).await {
                            Ok(plaintext) => match LanMessage::from_bytes(&plaintext) {
                                Ok(message) => {
                                    if bridge_tx
                                        .send(LanBridgeEvent::NetworkMessage(message, sender))
                                        .is_err()
                                    {
                                        warn!("LAN bridge closed while forwarding lobby message");
                                        break;
                                    }
                                }
                                Err(err) => {
                                    warn!("Failed to decode LAN message from {}: {}", sender, err);
                                }
                            },
                            Err(err) => {
                                warn!("Failed to decrypt LAN message from {}: {}", sender, err);
                            }
                        }
                    }
                    Err(err) => {
                        warn!("Socket receive error: {}", err);
                    }
                }
            }

            debug!("Socket receiver task stopped");
        });

        self.tasks.write().await.push(handle);
    }

    /// Request to create a new game
    pub async fn request_create_game(
        &self,
        game_name: String,
        is_direct_connect: bool,
    ) -> NetworkResult<()> {
        info!(
            "Creating game: {} (direct_connect: {})",
            game_name, is_direct_connect
        );

        if game_name.len() > self.config.max_game_name_length {
            return Err(NetworkError::invalid_command(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                self.config.max_game_name_length
            )));
        }

        let endpoint = self
            .local_endpoint
            .read()
            .await
            .as_ref()
            .copied()
            .ok_or_else(|| {
                NetworkError::configuration("Local endpoint not initialised".to_string())
            })?;

        if endpoint.ip().is_unspecified() {
            return Err(NetworkError::configuration(
                "Local IP unknown – set it via set_local_ip before hosting".to_string(),
            ));
        }

        let host_ip = endpoint.ip();
        let port = endpoint.port();

        let mut game = LanGameInfo::new(game_name.clone(), host_ip, port);
        game.is_direct_connect = is_direct_connect;
        game.is_public = !is_direct_connect;
        game.max_players = self.config.max_players;

        if let Some(public) = self.public_endpoint.read().await.as_ref().copied() {
            game.public_host = Some(public.ip());
            game.public_port = Some(public.port());
        }

        let mut host_player = LanPlayer::new_host(self.config.player_name.clone(), host_ip, port);
        host_player.login_name = self.config.login_name.clone();
        host_player.host_name = self.config.host_name.clone();

        game.add_player(host_player.clone())
            .map_err(|e| NetworkError::generic(format!("Failed to add host to game: {}", e)))?;

        *self.current_game.write().await = Some(game.clone());
        *self.local_player.write().await = Some(host_player);
        *self.is_hosting.write().await = true;
        *self.state.write().await = LobbyState::GameLobby;

        if self.networking_enabled {
            let announcement = self.build_announcement(&game);
            if let Err(err) = self.discovery.publish_local(announcement).await {
                warn!("Failed to publish discovery announcement: {}", err);
            }
            if let Err(err) = self.discovery.refresh_local(game.game_id).await {
                warn!("Failed to refresh discovery announcement: {}", err);
            }
        }
        *self.hosted_game_id.write().await = Some(game.game_id);

        if let Some(sender_info) = self.player_info().await {
            let lobby_announce = LanMessage::lobby_announce(sender_info.clone());
            self.broadcast_message(&lobby_announce).await?;

            let game_announce = LanMessage::game_announce(
                sender_info,
                game.game_id,
                game.name.clone(),
                matches!(game.state, GameState::Starting | GameState::InProgress),
                game.options.clone(),
                game.is_direct_connect,
                game.player_count,
                game.max_players,
                game.is_public,
                game.has_password,
                game.version_hash,
                game.map_crc,
            )
            .map_err(NetworkError::invalid_command)?;
            self.broadcast_message(&game_announce).await?;
        }

        let _ = self
            .bridge_tx
            .send(LanBridgeEvent::LobbyEvent(LobbyEvent::GameCreated(game)));

        info!("Game created successfully: {}", game_name);
        Ok(())
    }

    /// Request to join a game
    pub async fn request_join(
        &self,
        game: &LanGameInfo,
        target_ip: Option<IpAddr>,
    ) -> NetworkResult<()> {
        info!("Requesting to join game: {}", game.name);

        // Check if game is full
        if game.is_full() {
            return Err(NetworkError::invalid_command("Game is full".to_string()));
        }

        // Check if game has started
        if game.has_started() {
            return Err(NetworkError::invalid_command(
                "Game has already started".to_string(),
            ));
        }

        // Store join request
        let join_request = JoinRequest {
            game: game.clone(),
            target_ip,
            requested_at: NetworkInstant::now(),
        };
        *self.join_request.write().await = Some(join_request);

        if let Some(info) = self.player_info().await {
            let host_ip = target_ip.unwrap_or(game.host_ip);
            let request = LanMessage::request_join(info, host_ip, 0, 0, String::new())
                .map_err(NetworkError::invalid_command)?;
            let target_addr = SocketAddr::new(host_ip, self.config.base_port);
            self.send_to(&request, target_addr).await?;
            debug!("Join request sent for game: {}", game.name);
        } else {
            warn!("Join request ignored; local player info unavailable");
        }
        Ok(())
    }

    /// Request direct connect to an IP
    pub async fn request_direct_connect(&self, ip_address: IpAddr) -> NetworkResult<()> {
        info!("Requesting direct connect to: {}", ip_address);

        {
            let mut pending = self.pending_direct_connect.write().await;
            *pending = Some(DirectConnectState {
                target_ip: ip_address,
                requested_at: NetworkInstant::now(),
            });
        }

        if let Some(info) = self.player_info().await {
            let message = LanMessage::request_game_info(info, ip_address);
            let target = SocketAddr::new(ip_address, self.config.base_port);
            self.send_to(&message, target).await?;
            debug!("Direct connect info requested from {}", target);
        } else {
            warn!("Direct connect request skipped; no local player info");
        }

        Ok(())
    }

    /// Handle a direct-connect game announcement from a host.
    pub async fn maybe_handle_direct_connect(
        &self,
        game: &LanGameInfo,
        host_ip: IpAddr,
    ) -> NetworkResult<()> {
        let should_join = {
            let mut pending = self.pending_direct_connect.write().await;
            if let Some(state) = pending.as_ref() {
                if state.target_ip == host_ip || state.target_ip == game.host_ip {
                    *pending = None;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_join {
            info!(
                "Direct connect offer received from {}; requesting join",
                host_ip
            );
            self.request_join(game, Some(host_ip)).await?;
        }

        Ok(())
    }

    /// Request to leave current game/lobby
    pub async fn request_leave(&self) -> NetworkResult<()> {
        info!("Requesting to leave");

        let current_state = *self.state.read().await;
        match current_state {
            LobbyState::GameLobby | LobbyState::Starting | LobbyState::InGame => {
                let game_snapshot = { self.current_game.read().await.clone() };

                if let Some(game) = game_snapshot {
                    info!("Leaving game: {}", game.name);

                    if let Some(info) = self.player_info().await {
                        let leave = LanMessage::request_game_leave(info, game.name.clone())
                            .map_err(NetworkError::invalid_command)?;
                        if *self.is_hosting.read().await {
                            let _ = self.broadcast_message(&leave).await;
                        } else {
                            let target = SocketAddr::new(game.host_ip, self.config.base_port);
                            let _ = self.send_to(&leave, target).await;
                        }
                    }

                    {
                        let mut guard = self.current_game.write().await;
                        *guard = None;
                    }
                    *self.is_hosting.write().await = false;
                    if let Some(game_id) = self.hosted_game_id.write().await.take() {
                        if self.networking_enabled {
                            let _ = self.discovery.retract_local(game_id).await;
                        }
                    }
                    *self.join_request.write().await = None;
                    *self.game_start_timer.write().await = None;
                    *self.state.write().await = LobbyState::MainLobby;

                    info!("Left game successfully");
                }
            }
            LobbyState::MainLobby => {
                info!("Leaving main lobby");
                *self.state.write().await = LobbyState::None;
            }
            LobbyState::None => {
                debug!("Already not in lobby");
            }
        }

        Ok(())
    }

    /// Request lobby leave (different from game leave)
    pub async fn request_lobby_leave(&self, forced: bool) -> NetworkResult<()> {
        info!("Requesting lobby leave (forced: {})", forced);

        if forced {
            // Force leave everything
            *self.current_game.write().await = None;
            *self.is_hosting.write().await = false;
            if let Some(game_id) = self.hosted_game_id.write().await.take() {
                if self.networking_enabled {
                    let _ = self.discovery.retract_local(game_id).await;
                }
            }
            *self.join_request.write().await = None;
            *self.game_start_timer.write().await = None;
            *self.state.write().await = LobbyState::None;
        } else {
            // Normal leave
            self.request_leave().await?;
        }

        if let Some(info) = self.player_info().await {
            let message = LanMessage::request_lobby_leave(info);
            let _ = self.broadcast_message(&message).await;
        }

        Ok(())
    }

    /// Update acceptance of current game options
    pub async fn request_accept(&self, accepted: bool) -> NetworkResult<()> {
        info!("Setting accept state to {}", accepted);

        if let Some(ref mut local_player) = *self.local_player.write().await {
            local_player.set_accepted(accepted);
            let ip = local_player.ip;
            self.update_player_acceptance(ip, accepted).await;

            if let Some(game) = self.current_game.read().await.as_ref() {
                if let Some(info) = self.player_info().await {
                    let message = LanMessage::set_accept(info, game.name.clone(), accepted)
                        .map_err(NetworkError::invalid_command)?;
                    self.send_to_game_participants(&message).await?;
                }
            }

            self.emit_lobby_event(LobbyEvent::PlayerAccepted(local_player.ip, accepted))
                .await;
            debug!("Accept status sent");
        }

        Ok(())
    }

    /// Announce map availability state
    pub async fn request_map_status(&self, has_map: bool) -> NetworkResult<()> {
        info!("Announcing map availability: {}", has_map);

        if let Some(ref mut local_player) = *self.local_player.write().await {
            local_player.set_has_map(has_map);
            let ip = local_player.ip;
            self.update_player_map_status(ip, has_map).await;

            if let Some(game) = self.current_game.read().await.as_ref() {
                if let Some(info) = self.player_info().await {
                    let map_crc = game.map_crc.unwrap_or(0);
                    let message =
                        LanMessage::map_availability(info, game.name.clone(), map_crc, has_map)
                            .map_err(NetworkError::invalid_command)?;
                    self.send_to_game_participants(&message).await?;
                }
            }

            self.emit_lobby_event(LobbyEvent::PlayerMapStatus(local_player.ip, has_map))
                .await;
            debug!("Map status sent");
        }

        Ok(())
    }

    /// Request to start the game
    pub async fn request_game_start(&self) -> NetworkResult<()> {
        info!("Requesting game start");

        // Check if we're hosting
        if !*self.is_hosting.read().await {
            return Err(NetworkError::invalid_command(
                "Only host can start the game".to_string(),
            ));
        }

        // Check if all players are ready
        if let Some(ref game) = *self.current_game.read().await {
            if !game.all_players_accepted() {
                return Err(NetworkError::invalid_command(
                    "Not all players have accepted".to_string(),
                ));
            }

            if !game.all_players_have_map() {
                return Err(NetworkError::invalid_command(
                    "Not all players have the map".to_string(),
                ));
            }
        }

        // Update state
        *self.state.write().await = LobbyState::Starting;
        if let Some(info) = self.player_info().await {
            let message = LanMessage::game_start(info);
            self.send_to_remote_players(&message).await?;
        }

        self.set_game_start_timer_internal(None).await;
        info!("Game start initiated");
        Ok(())
    }

    /// Request game start with timer
    pub async fn request_game_start_timer(&self, seconds: u32) -> NetworkResult<()> {
        info!("Requesting game start timer: {} seconds", seconds);

        // Check if we're hosting
        if !*self.is_hosting.read().await {
            return Err(NetworkError::invalid_command(
                "Only host can start countdown".to_string(),
            ));
        }

        if let Some(info) = self.player_info().await {
            let message = LanMessage::game_start_timer(info, seconds);
            self.send_to_remote_players(&message).await?;
        }

        self.set_game_start_timer_internal(Some(seconds)).await;
        debug!("Game start timer set for {} seconds", seconds);
        Ok(())
    }

    /// Reset game start timer
    pub async fn reset_game_start_timer(&self) -> NetworkResult<()> {
        info!("Resetting game start timer");

        self.set_game_start_timer_internal(Some(0)).await;

        if *self.is_hosting.read().await {
            if let Some(info) = self.player_info().await {
                if self.current_game.read().await.is_some() {
                    let message = LanMessage::game_start_timer(info, 0);
                    self.send_to_remote_players(&message).await?;
                }
            }
        }

        debug!("Game start timer reset");
        Ok(())
    }

    /// Send a chat message to current participants
    pub async fn request_chat(&self, chat_type: ChatType, message: String) -> NetworkResult<()> {
        if message.trim().is_empty() {
            return Ok(());
        }

        if message.len() > self.config.max_chat_length {
            return Err(NetworkError::invalid_command(format!(
                "Chat message too long: {} > {}",
                message.len(),
                self.config.max_chat_length
            )));
        }

        let game = self.current_game.read().await.clone().ok_or_else(|| {
            NetworkError::invalid_command("No active game to send chat".to_string())
        })?;

        let info = self.player_info().await.ok_or_else(|| {
            NetworkError::invalid_command("Local player information unavailable".to_string())
        })?;

        let chat_message =
            LanMessage::chat(info.clone(), game.name.clone(), message.clone(), chat_type)
                .map_err(NetworkError::invalid_command)?;
        self.send_to_game_participants(&chat_message).await?;

        let mut chat_log = ChatMessage::new(info.name.clone(), info.ip, message.clone(), chat_type);
        chat_log.game_context = Some(game.name.clone());
        let _ = self.bridge_tx.send(LanBridgeEvent::ChatEvent(chat_log));

        debug!("Chat message sent: {}", message);
        Ok(())
    }

    /// Notify peers that the local player became inactive/active.
    pub async fn set_inactive(&self, inactive: bool) -> NetworkResult<()> {
        if inactive {
            if let Some(info) = self.player_info().await {
                let message = LanMessage::inactive(info);
                self.send_to_game_participants(&message).await?;
            }
            if let Some(ip) = self.local_player_ip().await {
                self.emit_lobby_event(LobbyEvent::PlayerInactive(ip)).await;
            }
        }
        Ok(())
    }

    /// Request to update game options
    pub async fn request_game_options(
        &self,
        options: GameOptions,
        is_public: bool,
        target_ip: Option<IpAddr>,
    ) -> NetworkResult<()> {
        info!("Requesting game options update");

        // Check if we're hosting
        if !*self.is_hosting.read().await {
            return Err(NetworkError::invalid_command(
                "Only host can change game options".to_string(),
            ));
        }

        let announce_after = {
            let mut guard = self.current_game.write().await;
            let game = guard
                .as_mut()
                .ok_or_else(|| NetworkError::invalid_command("No active game".to_string()))?;
            game.options = options.clone();
            game.is_public = is_public;

            // Reset all non-host acceptance state when options change
            for slot in &mut game.slots {
                if let Some(player) = slot.player.as_ref() {
                    if !player.is_host() {
                        slot.has_accepted = false;
                    }
                }
            }

            Some(game.clone())
        };

        if let Some(info) = self.player_info().await {
            let message = LanMessage::game_options(info, options.clone(), is_public)
                .map_err(NetworkError::invalid_command)?;

            if let Some(target_ip) = target_ip {
                let addr = SocketAddr::new(target_ip, self.config.base_port);
                self.send_to(&message, addr).await?;
            } else {
                self.send_to_remote_players(&message).await?;
            }
        }

        if self.networking_enabled {
            if let Some(game) = announce_after {
                let announcement = self.build_announcement(&game);
                if let Err(err) = self.discovery.publish_local(announcement).await {
                    warn!("Failed to update discovery announcement: {}", err);
                }
            }
        }

        self.emit_lobby_event(LobbyEvent::GameOptionsUpdated(options))
            .await;
        info!("Game options updated");
        Ok(())
    }

    /// Request to announce current game
    pub async fn request_announce(&self) -> NetworkResult<()> {
        info!("Requesting game announcement");

        if *self.is_hosting.read().await {
            if let Some(game) = self.current_game_snapshot().await {
                if let Some(info) = self.player_info().await {
                    let announce = LanMessage::game_announce(
                        info,
                        game.game_id,
                        game.name.clone(),
                        matches!(game.state, GameState::Starting | GameState::InProgress),
                        game.options.clone(),
                        game.is_direct_connect,
                        game.player_count,
                        game.max_players,
                        game.is_public,
                        game.has_password,
                        game.version_hash,
                        game.map_crc,
                    )
                    .map_err(NetworkError::invalid_command)?;
                    self.broadcast_message(&announce).await?;
                    if self.networking_enabled {
                        self.discovery.refresh_local(game.game_id).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Request name change
    pub async fn request_name_change(&self, new_name: String) -> NetworkResult<()> {
        info!("Requesting name change to: {}", new_name);

        if let Some(ref mut local_player) = *self.local_player.write().await {
            let old_name = local_player.name.clone();
            local_player.name = new_name.clone();
            if let Some(info) = self.player_info().await {
                let message = LanMessage::name_change(info, old_name.clone(), new_name.clone());
                let _ = self.send_to_game_participants(&message).await;
            }

            self.update_player_name(local_player.ip, &new_name).await;
            self.emit_lobby_event(LobbyEvent::NameChange(local_player.ip, new_name.clone()))
                .await;
            info!("Name changed from {} to {}", old_name, new_name);
        }

        Ok(())
    }

    /// Get current lobby state
    pub async fn get_state(&self) -> LobbyState {
        *self.state.read().await
    }

    /// Get current game
    pub async fn get_current_game(&self) -> Option<LanGameInfo> {
        self.current_game.read().await.clone()
    }

    /// Check if we're hosting
    pub async fn is_hosting(&self) -> bool {
        *self.is_hosting.read().await
    }

    /// Update lobby state
    pub async fn update(&mut self) -> NetworkResult<()> {
        // Check for timed out join requests
        let join_timeout = {
            let mut join_request = self.join_request.write().await;
            if let Some(ref request) = *join_request {
                if request.requested_at.elapsed() > Duration::from_secs(10) {
                    let game_name = request.game.name.clone();
                    let target = request.target_ip.or(Some(request.game.host_ip));
                    let target_repr = target
                        .map(|ip| ip.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(
                        "Join request timed out for game: {} (target: {})",
                        game_name, target_repr
                    );
                    *join_request = None;
                    Some((game_name, target))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((game_name, target)) = join_timeout {
            let message = if let Some(ip) = target {
                format!("Join request for {} at {} timed out", game_name, ip)
            } else {
                format!("Join request for {} timed out", game_name)
            };

            self.emit_lobby_event(LobbyEvent::Error(message)).await;
        }

        let direct_connect_timeout = {
            let mut pending = self.pending_direct_connect.write().await;
            if let Some(state) = pending.as_ref() {
                if state.requested_at.elapsed() > Duration::from_secs(10) {
                    let target = state.target_ip;
                    warn!("Direct connect request to {} timed out", target);
                    *pending = None;
                    Some(target)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(target) = direct_connect_timeout {
            self.emit_lobby_event(LobbyEvent::Error(format!(
                "Direct connect attempt to {} timed out",
                target
            )))
            .await;
        }

        // Update current game if we have one
        if let Some(ref mut game) = self.current_game.write().await.as_mut() {
            game.update_last_heard();
        }

        Ok(())
    }

    /// Shutdown the lobby
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down LAN lobby");

        *self.is_active.write().await = false;
        self.shutdown_notify.notify_waiters();

        // Wait for all tasks to complete
        let mut tasks = self.tasks.write().await;
        for handle in tasks.drain(..) {
            handle.abort();
            let _ = handle.await;
        }

        // Close socket
        *self.socket.write().await = None;

        // Clear state
        *self.current_game.write().await = None;
        *self.local_player.write().await = None;
        *self.is_hosting.write().await = false;
        if let Some(game_id) = self.hosted_game_id.write().await.take() {
            if self.networking_enabled {
                let _ = self.discovery.retract_local(game_id).await;
            }
        }
        *self.state.write().await = LobbyState::None;
        self.shutdown_notify = Arc::new(Notify::new());

        info!("LAN lobby shut down successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionManager;
    use crate::error::{NetworkError, NetworkResult};
    use crate::lan_api::{lan_event_channel, LanEventReceiver, LanEventSender, LanMessageType};
    use crate::security::encryption::{self, EncryptedPacket};
    use crate::security::SecurityManager;
    use crate::transport::Transport;
    use crate::DiscoveryConfig;
    use rustls::crypto::ring;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use tokio::net::UdpSocket;
    use tokio::sync::RwLock;
    use tokio::time::Duration;

    async fn build_lobby(base_port: u16) -> (LanLobby, LanEventReceiver) {
        let (tx, rx) = lan_event_channel();
        let discovery = Arc::new(
            GameDiscovery::new(
                DiscoveryConfig {
                    enable_mdns: false,
                    enable_broadcast: false,
                    broadcast_addr: Ipv4Addr::LOCALHOST,
                    mdns_service: "_test._udp.local".into(),
                    base_port,
                    resend_interval: Duration::from_secs(5),
                    stale_after: Duration::from_secs(30),
                },
                tx.clone(),
            )
            .await
            .unwrap(),
        );

        let mut config = LanConfig::default();
        config.base_port = base_port;
        config.broadcast_addr = Ipv4Addr::LOCALHOST;
        config.enable_broadcast = false;

        let lobby = LanLobby::new_test(config, discovery, tx).await.unwrap();
        (lobby, rx)
    }

    const HOST_ID: u8 = 0;
    const CLIENT_ID: u8 = 1;
    const HOST_NAME: &str = "EncryptedHost";
    const CLIENT_NAME: &str = "EncryptedClient";

    async fn setup_security_pair(
        host_id: u8,
        host_name: &str,
        client_id: u8,
        client_name: &str,
    ) -> NetworkResult<(Arc<SecurityManager>, Arc<SecurityManager>)> {
        let host_security = Arc::new(SecurityManager::new()?);
        let client_security = Arc::new(SecurityManager::new()?);

        let client_token = host_security.generate_auth_token(client_name);
        host_security
            .authenticate_player(
                client_id,
                client_name,
                client_token,
                client_security.identity_public_key().as_bytes().to_vec(),
            )
            .await?;

        let host_token = client_security.generate_auth_token(host_name);
        client_security
            .authenticate_player(
                host_id,
                host_name,
                host_token,
                host_security.identity_public_key().as_bytes().to_vec(),
            )
            .await?;

        let initiate = host_security.initiate_key_exchange(client_id).await?;
        let response = client_security
            .handle_key_exchange_initiate(initiate, host_id)
            .await?;
        let confirm = host_security.handle_key_exchange_response(response).await?;
        client_security.confirm_key_exchange(confirm).await?;

        host_security.secure_session_key(client_id).await?;
        client_security.secure_session_key(host_id).await?;

        Ok((host_security, client_security))
    }

    async fn build_connection_manager(
        security: Arc<SecurityManager>,
        local_id: u8,
    ) -> NetworkResult<Arc<RwLock<ConnectionManager>>> {
        let transport = Arc::new(Transport::new().await?);
        let mut manager = ConnectionManager::new_with_transport(transport).await?;
        manager.set_security_manager(security);
        manager.configure_local_endpoint(local_id, true, true);
        Ok(Arc::new(RwLock::new(manager)))
    }

    fn discovery_config(base_port: u16) -> DiscoveryConfig {
        DiscoveryConfig {
            enable_mdns: false,
            enable_broadcast: true,
            broadcast_addr: Ipv4Addr::LOCALHOST,
            mdns_service: format!("_encrypted_{}._udp.local", base_port),
            base_port,
            resend_interval: Duration::from_secs(1),
            stale_after: Duration::from_secs(3),
        }
    }

    fn make_lan_config(name: &str, port: u16) -> LanConfig {
        let mut config = LanConfig::default();
        config.player_name = name.to_string();
        config.login_name = format!("{}_login", name);
        config.host_name = format!("{}_host", name);
        config.base_port = port;
        config.broadcast_addr = Ipv4Addr::LOCALHOST;
        config.enable_broadcast = true;
        config
    }

    async fn build_secure_lobby_instance(
        config: LanConfig,
        discovery_config: DiscoveryConfig,
        security: Arc<SecurityManager>,
        connections: Arc<RwLock<ConnectionManager>>,
    ) -> NetworkResult<(
        LanLobby,
        Arc<GameDiscovery>,
        LanEventSender,
        LanEventReceiver,
    )> {
        let (bridge_tx, bridge_rx) = lan_event_channel();
        let discovery = Arc::new(
            GameDiscovery::with_dependencies(
                discovery_config,
                bridge_tx.clone(),
                Some(security.clone()),
                Some(connections.clone()),
            )
            .await?,
        );
        discovery.init().await?;

        let mut lobby = LanLobby::with_dependencies(
            config,
            discovery.clone(),
            bridge_tx.clone(),
            Some(security),
            Some(connections),
        )
        .await?;
        lobby.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
        lobby.init().await?;

        Ok((lobby, discovery, bridge_tx, bridge_rx))
    }

    #[tokio::test]
    async fn test_lobby_creation() {
        let (lobby, _rx) = build_lobby(9300).await;
        assert_eq!(lobby.get_state().await, LobbyState::None);
        assert!(!lobby.is_hosting().await);
        assert!(lobby.get_current_game().await.is_none());
    }

    #[tokio::test]
    async fn test_game_creation() {
        let (mut lobby, _rx) = build_lobby(9301).await;
        lobby.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
        lobby.init().await.unwrap();
        lobby
            .request_create_game("Test Game".into(), false)
            .await
            .unwrap();

        assert_eq!(lobby.get_state().await, LobbyState::GameLobby);
        assert!(lobby.is_hosting().await);
        let game = lobby.get_current_game().await.unwrap();
        assert_eq!(game.name, "Test Game");
        assert_eq!(game.player_count, 1);

        lobby.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_lobby_state_transitions() {
        let (mut lobby, _rx) = build_lobby(9302).await;
        lobby.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;

        assert_eq!(lobby.get_state().await, LobbyState::None);
        lobby.init().await.unwrap();
        assert_eq!(lobby.get_state().await, LobbyState::MainLobby);

        lobby
            .request_create_game("Test".into(), false)
            .await
            .unwrap();
        assert_eq!(lobby.get_state().await, LobbyState::GameLobby);

        lobby.request_leave().await.unwrap();
        assert_eq!(lobby.get_state().await, LobbyState::MainLobby);

        lobby.request_lobby_leave(false).await.unwrap();
        assert_eq!(lobby.get_state().await, LobbyState::None);

        lobby.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_game_start_validation() {
        let (mut lobby, _rx) = build_lobby(9303).await;
        lobby.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;

        lobby.init().await.unwrap();
        assert!(lobby.request_game_start().await.is_err());

        lobby
            .request_create_game("Test".into(), false)
            .await
            .unwrap();
        assert!(lobby.request_game_start().await.is_ok());
        assert_eq!(lobby.get_state().await, LobbyState::Starting);

        lobby.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn lobby_encrypts_outbound_payload_with_session_key() -> NetworkResult<()> {
        let _ = ring::default_provider().install_default();

        let capture_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let capture_addr = capture_socket.local_addr()?;

        let (host_security, client_security) =
            setup_security_pair(HOST_ID, HOST_NAME, CLIENT_ID, CLIENT_NAME).await?;
        let host_connections = build_connection_manager(host_security.clone(), HOST_ID).await?;

        {
            let guard = host_connections.read().await;
            guard.register_test_peer(CLIENT_ID, capture_addr).await;
        }

        let host_port = std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("ephemeral port")
            .local_addr()
            .unwrap()
            .port();
        let lan_config = make_lan_config(HOST_NAME, host_port);
        let discovery_config = discovery_config(lan_config.base_port);

        let (mut lobby, discovery, _bridge_tx, _bridge_rx) = build_secure_lobby_instance(
            lan_config,
            discovery_config,
            host_security.clone(),
            host_connections.clone(),
        )
        .await?;

        let lobby_endpoint = {
            let guard = lobby.local_endpoint.read().await;
            guard.expect("lobby local endpoint")
        };

        let player_info = PlayerInfo::new(
            HOST_NAME.to_string(),
            lobby_endpoint.ip(),
            lobby_endpoint.port(),
        );
        let message = LanMessage::request_locations(player_info);
        let plaintext = message.to_bytes().expect("serialize message");

        lobby.send_to(&message, capture_addr).await?;

        let mut buffer = vec![0u8; 2048];
        let (len, sender) = tokio::time::timeout(
            Duration::from_secs(1),
            capture_socket.recv_from(&mut buffer),
        )
        .await
        .expect("datagram timeout")?;
        assert_eq!(sender, lobby_endpoint);

        let envelope = encryption::decode_envelope(&buffer[..len])?;
        let packet = match envelope {
            encryption::Envelope::Encrypted {
                key_id,
                nonce,
                payload,
            } => {
                assert_eq!(key_id, 0, "session keys should map to key id 0");
                EncryptedPacket {
                    key_id,
                    nonce,
                    payload: payload.to_vec(),
                }
            }
            encryption::Envelope::Plain(_) => {
                panic!("expected encrypted envelope, got plain payload")
            }
        };

        let session_key = client_security.secure_session_key(HOST_ID).await?;
        let decrypted = client_security
            .encryption_provider()
            .decrypt_with_session(&packet, &session_key)
            .await?;
        assert_eq!(decrypted, plaintext);

        lobby.shutdown().await?;
        discovery.shutdown().await?;
        {
            let mut guard = host_connections.write().await;
            guard.shutdown_all().await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn lobby_decrypts_inbound_payload_with_session_key() -> NetworkResult<()> {
        let _ = ring::default_provider().install_default();

        let (host_security, client_security) =
            setup_security_pair(HOST_ID, HOST_NAME, CLIENT_ID, CLIENT_NAME).await?;
        let host_connections = build_connection_manager(host_security.clone(), HOST_ID).await?;

        let client_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let client_addr = client_socket.local_addr()?;

        {
            let guard = host_connections.read().await;
            guard.register_test_peer(CLIENT_ID, client_addr).await;
        }

        let host_port = std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("ephemeral port")
            .local_addr()
            .unwrap()
            .port();
        let lan_config = make_lan_config(HOST_NAME, host_port);
        let discovery_config = discovery_config(lan_config.base_port);

        let (mut lobby, discovery, _bridge_tx, mut bridge_rx) = build_secure_lobby_instance(
            lan_config,
            discovery_config,
            host_security.clone(),
            host_connections.clone(),
        )
        .await?;

        let lobby_addr = {
            let guard = lobby.local_endpoint.read().await;
            guard.expect("lobby endpoint")
        };

        let player_info = PlayerInfo::new(
            CLIENT_NAME.to_string(),
            client_addr.ip(),
            client_addr.port(),
        );
        let message = LanMessage::request_locations(player_info);
        let plaintext = message
            .to_bytes()
            .map_err(|err| NetworkError::generic(format!("serialize LAN message: {}", err)))?;

        let session_key = client_security.secure_session_key(HOST_ID).await?;
        let packet = client_security
            .encryption_provider()
            .encrypt(&plaintext, Some(session_key))
            .await?;
        let payload = encryption::encode_encrypted_envelope(&packet);

        client_socket.send_to(&payload, lobby_addr).await?;

        let (received_message, sender) = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                match bridge_rx.recv().await {
                    Some(LanBridgeEvent::NetworkMessage(message, sender)) => {
                        break (message, sender)
                    }
                    Some(_) => continue,
                    None => panic!("LAN bridge closed before message received"),
                }
            }
        })
        .await
        .map_err(|_| NetworkError::generic("timed out waiting for inbound message".to_string()))?;

        assert_eq!(sender, client_addr);
        assert!(matches!(
            received_message.message_type,
            LanMessageType::RequestLocations
        ));

        lobby.shutdown().await?;
        discovery.shutdown().await?;
        {
            let mut guard = host_connections.write().await;
            guard.shutdown_all().await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_game_start_timer() {
        let (mut lobby, mut rx) = build_lobby(9304).await;
        lobby.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;

        lobby.init().await.unwrap();
        lobby
            .request_create_game("Test".into(), false)
            .await
            .unwrap();
        lobby.request_game_start_timer(1).await.unwrap();

        tokio::time::sleep(Duration::from_millis(1100)).await;

        let mut saw_timer = false;
        let mut saw_start = false;
        while let Ok(message) = rx.try_recv() {
            if let LanBridgeEvent::LobbyEvent(event) = message {
                match event {
                    LobbyEvent::GameStartTimer(_) => saw_timer = true,
                    LobbyEvent::GameStarting => saw_start = true,
                    _ => {}
                }
            }
        }

        assert!(saw_timer);
        assert!(saw_start);

        lobby.shutdown().await.unwrap();
    }
}
