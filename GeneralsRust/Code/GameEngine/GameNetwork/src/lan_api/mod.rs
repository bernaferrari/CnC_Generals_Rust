//! Modern LAN API for Command & Conquer Generals Zero Hour
//!
//! This module provides a complete, async implementation of the original LANAPI
//! functionality using modern networking protocols and patterns. It supports:
//!
//! - Game discovery using UDP broadcast and mDNS
//! - Lobby and player management
//! - Chat functionality
//! - Game session coordination
//! - Host migration
//!
//! ## Architecture
//!
//! The LAN API is built around several core components:
//!
//! - [`LanApi`]: Main interface matching the original LANAPI functionality
//! - [`GameDiscovery`]: Network game discovery using modern protocols
//! - [`LanGameInfo`]: Game session information management
//! - [`LanPlayer`]: Player information and state tracking
//! - [`LanLobby`]: Lobby management and coordination
//! - [`LanChat`]: Chat system for LAN games
//!
//! ## Usage
//!
//! ```no_run
//! use game_network::lan_api::{LanApi, LanConfig, LanEvent};
//!
//! let rt = tokio::runtime::Runtime::new().unwrap();
//! rt.block_on(async {
//!     let config = LanConfig::default();
//!     let mut lan_api = LanApi::new(config).await.unwrap();
//!
//!     // Initialize LAN functionality
//!    lan_api.init().await.unwrap();
//!
//!     // Start game discovery
//!     lan_api.request_locations().await.unwrap();
//!
//!     // Create a game
//!     lan_api
//!         .request_game_create("My Game".to_string(), false)
//!         .await
//!         .unwrap();
//!
//!     // Process network events
//!     while let Some(event) = lan_api.update().await.unwrap() {
//!         match event {
//!             LanEvent::GameList(games) => {
//!                 println!("Found {} games", games.len());
//!             }
//!             LanEvent::PlayerJoin(player_name, slot) => {
//!                 println!("Player {} joined slot {}", player_name, slot);
//!             }
//!             _ => {}
//!         }
//!     }
//! });
//! ```

use crate::connection::ConnectionManager;
use crate::error::{NetworkError, NetworkResult};
use crate::nat::NatBinding;
use crate::security::SecurityManager;
use crate::time::NetworkInstant;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch, Mutex, RwLock};
use tracing::{debug, error, info, warn};

pub mod bus;
pub mod chat;
mod crypto;
pub mod discovery;
pub mod game_info;
pub mod lan_api;
pub mod lan_api_callbacks;
pub mod lan_api_handlers;
pub mod lan_game_info;
pub mod lanap_ihandlers;
pub mod lanapi;
pub mod lanapi_callbacks;
pub mod lobby;
pub mod messages;
pub mod player;

pub use bus::{lan_event_channel, LanBridgeEvent, LanEventReceiver, LanEventSender};
pub use chat::{ChatMessage, ChatType, LanChat};
pub use discovery::{DiscoveryConfig, DiscoveryMethod, GameDiscovery};
pub use game_info::{GameOptions, GameState as LanGameState, LanGameInfo};
pub use lobby::{LanLobby, LobbyEvent, LobbyState};
pub use messages::{LanMessage, LanMessageType, MessagePayload};
pub use player::{LanPlayer, PlayerRole, PlayerState};

/// LAN API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanConfig {
    /// Local player name (max 12 characters)
    pub player_name: String,
    /// Login name (legacy compatibility)
    pub login_name: String,
    /// Host/machine name (legacy compatibility)
    pub host_name: String,
    /// Base UDP port for LAN communication
    pub base_port: u16,
    /// Broadcast address for game discovery
    pub broadcast_addr: Ipv4Addr,
    /// mDNS service name for modern discovery
    pub mdns_service: String,
    /// Action timeout duration
    pub action_timeout: Duration,
    /// Resend interval for reliability
    pub resend_interval: Duration,
    /// Maximum number of players per game
    pub max_players: u8,
    /// Enable modern discovery methods (mDNS)
    pub enable_mdns: bool,
    /// Enable legacy UDP broadcast discovery
    pub enable_broadcast: bool,
    /// Enable host migration
    pub enable_host_migration: bool,
    /// Maximum chat message length
    pub max_chat_length: usize,
    /// Maximum game name length
    pub max_game_name_length: usize,
}

impl Default for LanConfig {
    fn default() -> Self {
        Self {
            player_name: "Player".to_string(),
            login_name: "".to_string(),
            host_name: "".to_string(),
            base_port: 8086,
            broadcast_addr: Ipv4Addr::BROADCAST,
            mdns_service: "_generals._udp.local".to_string(),
            action_timeout: Duration::from_secs(5),
            resend_interval: Duration::from_secs(10),
            max_players: 8,
            enable_mdns: true,
            enable_broadcast: true,
            enable_host_migration: true,
            max_chat_length: 100,
            max_game_name_length: 16,
        }
    }
}

/// LAN API events that applications can handle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LanEvent {
    /// List of discovered games
    GameList(Vec<LanGameInfo>),
    /// List of players in lobby
    PlayerList(Vec<LanPlayer>),
    /// Game join result
    GameJoin(LanResult, Option<LanGameInfo>),
    /// Player joined the game
    PlayerJoin(String, u8), // player_name, slot
    /// Host left the game
    HostLeave,
    /// Player left the game
    PlayerLeave(String), // player_name
    /// Player's accept status changed
    AcceptStatus(IpAddr, bool),
    /// Player's map availability status
    MapStatus(IpAddr, bool),
    /// Chat message received
    Chat(String, IpAddr, String, ChatType), // player, ip, message, type
    /// Game is starting
    GameStart,
    /// Game start countdown
    GameStartTimer(u32), // seconds
    /// Game options updated
    GameOptions(IpAddr, u8, GameOptions), // player_ip, slot, options
    /// Game options updated (alternative name for compatibility)
    GameOptionsUpdated(GameOptions),
    /// Game created successfully
    GameCreate(LanResult),
    /// Player changed name
    NameChange(IpAddr, String), // ip, new_name
    /// Player went inactive
    PlayerInactive(IpAddr),
    /// Network error occurred
    NetworkError(String),
}

/// LAN API result codes matching the original C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanResult {
    Ok,
    Timeout,
    GameFull,
    DuplicateName,
    CrcMismatch,
    SerialDupe,
    GameStarted,
    GameExists,
    GameGone,
    Busy,
    Unknown,
}

impl std::fmt::Display for LanResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanResult::Ok => write!(f, "OK"),
            LanResult::Timeout => write!(f, "Timeout"),
            LanResult::GameFull => write!(f, "Game Full"),
            LanResult::DuplicateName => write!(f, "Duplicate Name"),
            LanResult::CrcMismatch => write!(f, "CRC Mismatch"),
            LanResult::SerialDupe => write!(f, "Serial Duplicate"),
            LanResult::GameStarted => write!(f, "Game Started"),
            LanResult::GameExists => write!(f, "Game Exists"),
            LanResult::GameGone => write!(f, "Game Gone"),
            LanResult::Busy => write!(f, "Busy"),
            LanResult::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Current action being performed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    None,
    Join,
    JoinDirectConnect,
    Leave,
    CreateGame,
}

/// Main LAN API interface
pub struct LanApi {
    /// Configuration
    config: LanConfig,
    /// Game discovery service
    discovery: Arc<GameDiscovery>,
    /// Current lobby state
    lobby: Arc<RwLock<LanLobby>>,
    /// Chat system
    chat: Arc<RwLock<LanChat>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<LanEvent>,
    /// Event receiver for external consumers
    event_rx: broadcast::Receiver<LanEvent>,
    /// Internal bridge used by subsystems to relay events into the API
    bridge_tx: LanEventSender,
    bridge_rx: Arc<Mutex<LanEventReceiver>>,
    /// Currently discovered games
    games: Arc<RwLock<HashMap<String, LanGameInfo>>>,
    /// Current lobby players
    lobby_players: Arc<RwLock<HashMap<IpAddr, LanPlayer>>>,
    /// Current game we're in
    current_game: Arc<RwLock<Option<LanGameInfo>>>,
    /// Local IP address
    local_ip: Arc<RwLock<Option<IpAddr>>>,
    /// Whether we're currently hosting
    is_host: Arc<RwLock<bool>>,
    /// Current pending action
    pending_action: Arc<RwLock<PendingAction>>,
    /// Action expiration time
    action_expiration: Arc<RwLock<Option<NetworkInstant>>>,
    /// Whether the application is currently active
    is_active: Arc<RwLock<bool>>,
    /// Background task handle
    bg_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// NAT binding watch task handle
    nat_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl LanApi {
    /// Create a new LAN API instance
    pub async fn new(config: LanConfig) -> NetworkResult<Self> {
        Self::with_dependencies(config, None, None).await
    }

    pub async fn with_dependencies(
        config: LanConfig,
        security: Option<Arc<SecurityManager>>,
        connections: Option<Arc<RwLock<ConnectionManager>>>,
    ) -> NetworkResult<Self> {
        info!("Creating LAN API with config: {:?}", config);

        // Validate configuration
        if config.player_name.len() > config.max_game_name_length {
            return Err(NetworkError::configuration(format!(
                "Player name too long: {} > {}",
                config.player_name.len(),
                config.max_game_name_length
            )));
        }

        // Create event channel
        let (event_tx, event_rx) = broadcast::channel(1000);

        // Create bridge bus between subsystems and the public API.
        let (bridge_tx, bridge_rx) = lan_event_channel();

        // Initialize discovery service
        let discovery_config = DiscoveryConfig {
            enable_mdns: config.enable_mdns,
            enable_broadcast: config.enable_broadcast,
            broadcast_addr: config.broadcast_addr,
            mdns_service: config.mdns_service.clone(),
            base_port: config.base_port,
            resend_interval: config.resend_interval,
            stale_after: config.resend_interval.mul_f32(3.0),
        };
        let discovery = Arc::new(
            GameDiscovery::with_dependencies(
                discovery_config,
                bridge_tx.clone(),
                security.clone(),
                connections.clone(),
            )
            .await?,
        );

        // Initialize lobby
        let lobby = Arc::new(RwLock::new(
            LanLobby::with_dependencies(
                config.clone(),
                discovery.clone(),
                bridge_tx.clone(),
                security.clone(),
                connections.clone(),
            )
            .await?,
        ));

        // Initialize chat
        let chat = Arc::new(RwLock::new(
            LanChat::with_dependencies(
                config.max_chat_length,
                config.base_port,
                bridge_tx.clone(),
                security.clone(),
                connections,
            )
            .await?,
        ));

        Ok(Self {
            config,
            discovery,
            lobby,
            chat,
            event_tx,
            event_rx,
            bridge_tx,
            bridge_rx: Arc::new(Mutex::new(bridge_rx)),
            games: Arc::new(RwLock::new(HashMap::new())),
            lobby_players: Arc::new(RwLock::new(HashMap::new())),
            current_game: Arc::new(RwLock::new(None)),
            local_ip: Arc::new(RwLock::new(None)),
            is_host: Arc::new(RwLock::new(false)),
            pending_action: Arc::new(RwLock::new(PendingAction::None)),
            action_expiration: Arc::new(RwLock::new(None)),
            is_active: Arc::new(RwLock::new(true)),
            bg_task: Arc::new(RwLock::new(None)),
            nat_task: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize the LAN API
    pub async fn init(&mut self) -> NetworkResult<()> {
        info!("Initializing LAN API");

        // Initialize discovery
        self.discovery.init().await?;

        // Initialize lobby
        {
            let mut lobby = self.lobby.write().await;
            lobby.init().await?;
        }

        // Initialize chat
        {
            let mut chat = self.chat.write().await;
            chat.init().await?;
        }

        // Start background task
        self.start_background_task().await;

        info!("LAN API initialized successfully");
        Ok(())
    }

    /// Attach NAT binding updates so public announcements stay in sync with external reachability.
    pub async fn attach_nat_updates(
        &self,
        mut updates: watch::Receiver<Option<NatBinding>>,
    ) -> NetworkResult<()> {
        Self::apply_nat_binding(
            Arc::clone(&self.discovery),
            Arc::clone(&self.lobby),
            updates.borrow().clone(),
        )
        .await?;

        let discovery = Arc::clone(&self.discovery);
        let lobby = Arc::clone(&self.lobby);
        let handle = tokio::spawn(async move {
            loop {
                if updates.changed().await.is_err() {
                    break;
                }

                let binding = updates.borrow().clone();
                if let Err(err) =
                    Self::apply_nat_binding(Arc::clone(&discovery), Arc::clone(&lobby), binding)
                        .await
                {
                    warn!("Failed to apply NAT update to LAN subsystems: {}", err);
                }
            }
            debug!("NAT-to-LAN bridge task terminating");
        });

        if let Some(existing) = self.nat_task.write().await.replace(handle) {
            existing.abort();
            let _ = existing.await;
        }

        Ok(())
    }

    /// Start the background processing task
    async fn start_background_task(&self) {
        let bridge_rx = Arc::clone(&self.bridge_rx);
        let event_tx = self.event_tx.clone();
        let games = Arc::clone(&self.games);
        let lobby_players = Arc::clone(&self.lobby_players);
        let current_game = Arc::clone(&self.current_game);
        let pending_action = Arc::clone(&self.pending_action);
        let action_expiration = Arc::clone(&self.action_expiration);
        let is_host = Arc::clone(&self.is_host);
        let lobby = Arc::clone(&self.lobby);

        let handle = tokio::spawn(async move {
            let mut bridge_rx = bridge_rx.lock().await;

            loop {
                tokio::select! {
                    // Process internal messages
                    msg = bridge_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if let Err(e) = Self::handle_bridge_event(
                                    &msg,
                                    &event_tx,
                                    &games,
                                    &lobby_players,
                                    &current_game,
                                    &pending_action,
                                    &action_expiration,
                                    &is_host,
                                    &lobby,
                                ).await {
                                    error!("Error handling internal message: {}", e);
                                }

                                if matches!(msg, LanBridgeEvent::Shutdown) {
                                    break;
                                }
                            }
                            None => {
                                warn!("Internal message channel closed");
                                break;
                            }
                        }
                    }

                    // Check for action timeouts
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        let now = NetworkInstant::now();
                        let mut action_expiration = action_expiration.write().await;
                        let mut pending_action_guard = pending_action.write().await;

                        if let Some(expiry) = *action_expiration {
                            if now >= expiry {
                                let expired_action = *pending_action_guard;
                                *pending_action_guard = PendingAction::None;
                                *action_expiration = None;

                                // Send timeout event
                                match expired_action {
                                    PendingAction::Join | PendingAction::JoinDirectConnect => {
                                        let _ = event_tx.send(LanEvent::GameJoin(LanResult::Timeout, None));
                                    }
                                    PendingAction::CreateGame => {
                                        let _ = event_tx.send(LanEvent::GameCreate(LanResult::Timeout));
                                    }
                                    _ => {}
                                }

                                debug!("Action {:?} timed out", expired_action);
                            }
                        }
                    }
                }
            }

            info!("LAN API background task stopped");
        });

        *self.bg_task.write().await = Some(handle);
    }

    /// Handle bridge events emitted by LAN subsystems.
    async fn handle_bridge_event(
        msg: &LanBridgeEvent,
        event_tx: &broadcast::Sender<LanEvent>,
        games: &Arc<RwLock<HashMap<String, LanGameInfo>>>,
        lobby_players: &Arc<RwLock<HashMap<IpAddr, LanPlayer>>>,
        current_game: &Arc<RwLock<Option<LanGameInfo>>>,
        pending_action: &Arc<RwLock<PendingAction>>,
        action_expiration: &Arc<RwLock<Option<NetworkInstant>>>,
        is_host: &Arc<RwLock<bool>>,
        lobby: &Arc<RwLock<LanLobby>>,
    ) -> NetworkResult<()> {
        match msg {
            LanBridgeEvent::NetworkMessage(message, sender) => {
                debug!(
                    "Handling network message from {}: {:?}",
                    sender, message.message_type
                );
                Self::handle_network_message(
                    message,
                    *sender,
                    event_tx,
                    games,
                    lobby_players,
                    current_game,
                    pending_action,
                    action_expiration,
                    is_host,
                    lobby,
                )
                .await?;
            }
            LanBridgeEvent::DiscoverySnapshot(discovered_games) => {
                let mut games_guard = games.write().await;
                games_guard.clear();
                for game in discovered_games {
                    games_guard.insert(game.name.clone(), game.clone());
                }
                let _ = event_tx.send(LanEvent::GameList(discovered_games.clone()));
            }
            LanBridgeEvent::LobbyEvent(lobby_event) => {
                debug!("Lobby update: {:?}", lobby_event);
                match lobby_event.clone() {
                    LobbyEvent::GameCreated(game) => {
                        *is_host.write().await = true;
                        *current_game.write().await = Some(game.clone());
                        let mut games_guard = games.write().await;
                        games_guard.insert(game.name.clone(), game.clone());
                        let _ = event_tx.send(LanEvent::GameCreate(LanResult::Ok));
                        let snapshot: Vec<LanGameInfo> = games_guard.values().cloned().collect();
                        drop(games_guard);
                        let _ = event_tx.send(LanEvent::GameList(snapshot));
                    }
                    LobbyEvent::PlayerJoined(player) => {
                        lobby_players
                            .write()
                            .await
                            .insert(player.ip, player.clone());
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ =
                            event_tx.send(LanEvent::PlayerJoin(player.name.clone(), player.team));
                    }
                    LobbyEvent::PlayerLeft(ip, name) => {
                        lobby_players.write().await.remove(&ip);
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ = event_tx.send(LanEvent::PlayerLeave(name.clone()));
                    }
                    LobbyEvent::GameStartTimer(seconds) => {
                        let _ = event_tx.send(LanEvent::GameStartTimer(seconds));
                    }
                    LobbyEvent::GameStarting => {
                        let _ = event_tx.send(LanEvent::GameStart);
                    }
                    LobbyEvent::GameOptionsUpdated(options) => {
                        if let Some(ref mut game) = *current_game.write().await {
                            game.options = options.clone();
                        }
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ = event_tx.send(LanEvent::GameOptionsUpdated(options.clone()));
                    }
                    LobbyEvent::PlayerAccepted(ip, accepted) => {
                        if let Some(ref mut game) = *current_game.write().await {
                            game.set_player_accepted(ip, accepted);
                        }
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ = event_tx.send(LanEvent::AcceptStatus(ip, accepted));
                    }
                    LobbyEvent::PlayerMapStatus(ip, has_map) => {
                        if let Some(ref mut game) = *current_game.write().await {
                            game.set_player_has_map(ip, has_map);
                        }
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ = event_tx.send(LanEvent::MapStatus(ip, has_map));
                    }
                    LobbyEvent::Error(reason) => {
                        let _ = event_tx.send(LanEvent::NetworkError(reason.clone()));
                    }
                    LobbyEvent::NameChange(ip_addr, name) => {
                        {
                            let mut players = lobby_players.write().await;
                            if let Some(player) = players.get_mut(&ip_addr) {
                                player.name = name.clone();
                            }
                        }
                        if let Some(ref mut game) = *current_game.write().await {
                            if let Some(slot) = game
                                .slots
                                .iter_mut()
                                .find(|slot| slot.player.as_ref().map(|p| p.ip) == Some(ip_addr))
                            {
                                if let Some(player) = slot.player.as_mut() {
                                    player.name = name.clone();
                                }
                            }
                        }
                        if let Some(game) = current_game.read().await.clone() {
                            let mut guard = games.write().await;
                            guard.insert(game.name.clone(), game);
                        }
                        let _ = event_tx.send(LanEvent::NameChange(ip_addr, name.clone()));
                    }
                    _ => {}
                }
            }
            LanBridgeEvent::ChatEvent(chat_msg) => {
                let _ = event_tx.send(LanEvent::Chat(
                    chat_msg.sender_name.clone(),
                    chat_msg.sender_ip,
                    chat_msg.message.clone(),
                    chat_msg.chat_type,
                ));
            }
            LanBridgeEvent::Shutdown => {
                info!("Shutting down background task");
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_network_message(
        message: &LanMessage,
        sender: SocketAddr,
        event_tx: &broadcast::Sender<LanEvent>,
        games: &Arc<RwLock<HashMap<String, LanGameInfo>>>,
        lobby_players: &Arc<RwLock<HashMap<IpAddr, LanPlayer>>>,
        current_game: &Arc<RwLock<Option<LanGameInfo>>>,
        pending_action: &Arc<RwLock<PendingAction>>,
        action_expiration: &Arc<RwLock<Option<NetworkInstant>>>,
        is_host: &Arc<RwLock<bool>>,
        lobby: &Arc<RwLock<LanLobby>>,
    ) -> NetworkResult<()> {
        let sender_ip = sender.ip();
        let sender_port = sender.port();

        let remote_player = |state: PlayerState| {
            let mut player = LanPlayer::new(message.sender.name.clone(), sender_ip, sender_port);
            player.login_name = message.sender.login_name.clone();
            player.host_name = message.sender.host_name.clone();
            player.state = state;
            player
        };

        match message.message_type {
            LanMessageType::RequestLocations => {
                let lobby_guard = Arc::clone(&lobby).read_owned().await;
                if let Some(info) = lobby_guard.player_info().await {
                    let lobby_msg = LanMessage::lobby_announce(info.clone());
                    lobby_guard.send_to(&lobby_msg, sender).await?;
                    if *is_host.read().await {
                        if let Some(game) = lobby_guard.current_game_snapshot().await {
                            let announce = LanMessage::game_announce(
                                info.clone(),
                                game.game_id,
                                game.name.clone(),
                                matches!(
                                    game.state,
                                    LanGameState::Starting | LanGameState::InProgress
                                ),
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
                            lobby_guard.send_to(&announce, sender).await?;
                        }
                    }
                }
            }
            LanMessageType::LobbyAnnounce => {
                let player = remote_player(PlayerState::InLobby);
                {
                    let mut players = lobby_players.write().await;
                    players.insert(player.ip, player.clone());
                    let snapshot: Vec<LanPlayer> = players.values().cloned().collect();
                    let _ = event_tx.send(LanEvent::PlayerList(snapshot));
                }
            }
            LanMessageType::GameAnnounce => {
                if let MessagePayload::GameInfo {
                    game_id,
                    game_name,
                    in_progress,
                    options,
                    is_direct_connect,
                    player_count,
                    max_players,
                    is_public,
                    has_password,
                    version_hash,
                    map_crc,
                } = &message.payload
                {
                    match GameOptions::from_string(options) {
                        Ok(parsed_options) => {
                            let mut game =
                                LanGameInfo::new(game_name.clone(), sender_ip, sender_port);
                            game.game_id = *game_id;
                            game.options = parsed_options;
                            game.is_direct_connect = *is_direct_connect;
                            game.player_count = *player_count;
                            game.max_players = *max_players;
                            game.has_password = *has_password;
                            game.is_public = *is_public;
                            game.version_hash = *version_hash;
                            game.map_crc = *map_crc;
                            game.last_heard = Utc::now();
                            game.state = if *in_progress {
                                LanGameState::InProgress
                            } else {
                                LanGameState::Lobby
                            };

                            {
                                let mut guard = games.write().await;
                                guard.insert(game.name.clone(), game.clone());
                                let snapshot: Vec<LanGameInfo> = guard.values().cloned().collect();
                                let _ = event_tx.send(LanEvent::GameList(snapshot));
                            }

                            let lobby_guard = Arc::clone(&lobby).read_owned().await;
                            lobby_guard
                                .maybe_handle_direct_connect(&game, sender_ip)
                                .await?;
                        }
                        Err(err) => {
                            warn!("Failed to parse game options from announcement: {}", err)
                        }
                    }
                }
            }
            LanMessageType::RequestJoin => {
                if !*is_host.read().await {
                    return Ok(());
                }

                if let MessagePayload::GameToJoin {
                    exe_crc,
                    ini_crc,
                    serial_hash,
                    player_name,
                    ..
                } = &message.payload
                {
                    let mut new_player = remote_player(PlayerState::InGameSetup);
                    new_player.name = player_name.clone();
                    new_player.exe_crc = Some(*exe_crc);
                    new_player.ini_crc = Some(*ini_crc);
                    new_player.serial_hash = Some(serial_hash.clone());

                    let lobby_guard = Arc::clone(&lobby).read_owned().await;
                    if let Some(host_info) = lobby_guard.player_info().await {
                        match lobby_guard
                            .add_player_to_current_game(new_player.clone())
                            .await
                        {
                            Ok((updated_game, slot)) => {
                                let accept = LanMessage::join_accept(
                                    host_info,
                                    updated_game.name.clone(),
                                    sender_ip,
                                    slot,
                                    updated_game.game_id,
                                )
                                .map_err(NetworkError::invalid_command)?;
                                lobby_guard.send_to(&accept, sender).await?;

                                let mut joined_player = new_player.clone();
                                joined_player.team = slot;
                                joined_player.state = PlayerState::InGameSetup;

                                {
                                    let mut players = lobby_players.write().await;
                                    players.insert(sender_ip, joined_player.clone());
                                }

                                *current_game.write().await = Some(updated_game.clone());
                                {
                                    let mut guard = games.write().await;
                                    guard.insert(updated_game.name.clone(), updated_game);
                                }

                                let _ = event_tx
                                    .send(LanEvent::PlayerJoin(joined_player.name.clone(), slot));
                            }
                            Err(err) => {
                                let deny = LanMessage::join_deny(
                                    host_info,
                                    message.sender.name.clone(),
                                    sender_ip,
                                    LanResult::GameFull,
                                )
                                .map_err(NetworkError::invalid_command)?;
                                lobby_guard.send_to(&deny, sender).await?;
                                warn!("Join request denied: {}", err);
                            }
                        }
                    }
                }
            }
            LanMessageType::JoinAccept => {
                if let MessagePayload::GameJoined {
                    game_name,
                    game_ip,
                    slot_position,
                    game_id,
                    ..
                } = &message.payload
                {
                    *pending_action.write().await = PendingAction::None;
                    *action_expiration.write().await = None;
                    *is_host.write().await = false;

                    let mut guard = games.write().await;
                    let mut game = guard.get(game_name).cloned().unwrap_or_else(|| {
                        LanGameInfo::new(game_name.clone(), *game_ip, sender_port)
                    });
                    game.game_id = *game_id;
                    game.host_ip = *game_ip;
                    game.port = sender_port;
                    game.state = LanGameState::Lobby;
                    if game.player_count <= *slot_position {
                        game.player_count = slot_position + 1;
                    }
                    guard.insert(game_name.clone(), game.clone());
                    drop(guard);

                    *current_game.write().await = Some(game.clone());
                    let _ = event_tx.send(LanEvent::GameJoin(LanResult::Ok, Some(game)));
                }
            }
            LanMessageType::JoinDeny => {
                if let MessagePayload::GameNotJoined { reason, .. } = &message.payload {
                    *pending_action.write().await = PendingAction::None;
                    *action_expiration.write().await = None;
                    let _ = event_tx.send(LanEvent::GameJoin(*reason, None));
                }
            }
            LanMessageType::RequestGameLeave => {
                if let Some(updated_game) = lobby
                    .clone()
                    .read_owned()
                    .await
                    .remove_player_from_current_game(sender_ip)
                    .await
                {
                    {
                        let mut players = lobby_players.write().await;
                        players.remove(&sender_ip);
                        let snapshot: Vec<LanPlayer> = players.values().cloned().collect();
                        let _ = event_tx.send(LanEvent::PlayerList(snapshot));
                    }
                    *current_game.write().await = Some(updated_game.clone());
                    {
                        let mut guard = games.write().await;
                        guard.insert(updated_game.name.clone(), updated_game.clone());
                    }
                    let _ = event_tx.send(LanEvent::PlayerLeave(message.sender.name.clone()));
                }
            }
            LanMessageType::MapAvailability => {
                if let MessagePayload::MapStatus { has_map, .. } = &message.payload {
                    lobby
                        .clone()
                        .read_owned()
                        .await
                        .update_player_map_status(sender_ip, *has_map)
                        .await;
                    if let Some(game) = current_game.read().await.clone() {
                        let mut guard = games.write().await;
                        guard.insert(game.name.clone(), game);
                    }
                    let _ = event_tx.send(LanEvent::MapStatus(sender_ip, *has_map));
                }
            }
            LanMessageType::SetAccept => {
                if let MessagePayload::Accept { is_accepted, .. } = &message.payload {
                    lobby
                        .clone()
                        .read_owned()
                        .await
                        .update_player_acceptance(sender_ip, *is_accepted)
                        .await;
                    if let Some(game) = current_game.read().await.clone() {
                        let mut guard = games.write().await;
                        guard.insert(game.name.clone(), game);
                    }
                    let _ = event_tx.send(LanEvent::AcceptStatus(sender_ip, *is_accepted));
                }
            }
            LanMessageType::GameStartTimer => {
                if let MessagePayload::StartTimer { seconds } = &message.payload {
                    let lobby_guard = lobby.clone().read_owned().await;
                    lobby_guard
                        .set_game_start_timer_internal(Some(*seconds))
                        .await;
                    if let Some(ref mut game) = *current_game.write().await {
                        game.state = LanGameState::Starting;
                    }
                    if let Some(game) = current_game.read().await.clone() {
                        let mut guard = games.write().await;
                        guard.insert(game.name.clone(), game);
                    }
                }
            }
            LanMessageType::GameStart => {
                let lobby_guard = Arc::clone(&lobby).read_owned().await;
                lobby_guard.set_game_start_timer_internal(None).await;
                if let Some(ref mut game) = *current_game.write().await {
                    game.state = LanGameState::InProgress;
                }
                if let Some(game) = current_game.read().await.clone() {
                    let mut guard = games.write().await;
                    guard.insert(game.name.clone(), game);
                }
            }
            LanMessageType::GameOptions => {
                if let MessagePayload::GameOptions { options, is_public } = &message.payload {
                    match GameOptions::from_string(options) {
                        Ok(parsed) => {
                            let lobby_guard = Arc::clone(&lobby).read_owned().await;
                            lobby_guard
                                .apply_remote_game_options(&parsed, *is_public)
                                .await;
                            if let Some(ref mut game) = *current_game.write().await {
                                game.options = parsed.clone();
                                game.is_public = *is_public;
                            }
                            if let Some(game) = current_game.read().await.clone() {
                                let mut guard = games.write().await;
                                guard.insert(game.name.clone(), game);
                            }
                            let _ = event_tx.send(LanEvent::GameOptionsUpdated(parsed));
                        }
                        Err(err) => warn!("Failed to parse remote game options: {}", err),
                    }
                }
            }
            LanMessageType::RequestGameInfo => {
                if *is_host.read().await {
                    let lobby_guard = Arc::clone(&lobby).read_owned().await;
                    if let Some(info) = lobby_guard.player_info().await {
                        if let Some(game) = lobby_guard.current_game_snapshot().await {
                            let announce = LanMessage::game_announce(
                                info.clone(),
                                game.game_id,
                                game.name.clone(),
                                matches!(
                                    game.state,
                                    LanGameState::Starting | LanGameState::InProgress
                                ),
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
                            lobby_guard.send_to(&announce, sender).await?;

                            let options_msg = LanMessage::game_options(
                                info.clone(),
                                game.options.clone(),
                                game.is_public,
                            )
                            .map_err(NetworkError::invalid_command)?;
                            lobby_guard.send_to(&options_msg, sender).await?;

                            let map_msg = LanMessage::map_availability(
                                info.clone(),
                                game.name.clone(),
                                game.map_crc.unwrap_or(0),
                                true,
                            )
                            .map_err(NetworkError::invalid_command)?;
                            lobby_guard.send_to(&map_msg, sender).await?;

                            let accept_msg =
                                LanMessage::set_accept(info.clone(), game.name.clone(), true)
                                    .map_err(NetworkError::invalid_command)?;
                            lobby_guard.send_to(&accept_msg, sender).await?;
                        }
                    }
                }
            }
            LanMessageType::NameChange => {
                if let MessagePayload::NameChange { new_name, .. } = &message.payload {
                    let lobby_guard = lobby.clone().read_owned().await;
                    lobby_guard.update_player_name(sender_ip, new_name).await;
                    {
                        let mut players = lobby_players.write().await;
                        if let Some(player) = players.get_mut(&sender_ip) {
                            player.name = new_name.clone();
                        }
                    }
                    if let Some(ref mut game) = *current_game.write().await {
                        if let Some(slot) = game
                            .slots
                            .iter_mut()
                            .find(|slot| slot.player.as_ref().map(|p| p.ip) == Some(sender_ip))
                        {
                            if let Some(player) = slot.player.as_mut() {
                                player.name = new_name.clone();
                            }
                        }
                    }
                    let _ = event_tx.send(LanEvent::NameChange(sender_ip, new_name.clone()));
                }
            }
            LanMessageType::RequestLobbyLeave => {
                if let Some(updated_game) = lobby
                    .clone()
                    .read_owned()
                    .await
                    .remove_player_from_current_game(sender_ip)
                    .await
                {
                    *current_game.write().await = Some(updated_game.clone());
                    {
                        let mut guard = games.write().await;
                        guard.insert(updated_game.name.clone(), updated_game);
                    }
                }
                {
                    let mut players = lobby_players.write().await;
                    players.remove(&sender_ip);
                }
                let _ = event_tx.send(LanEvent::PlayerLeave(message.sender.name.clone()));
            }
            LanMessageType::Chat => {
                if let MessagePayload::Chat {
                    game_name: _,
                    chat_type,
                    message: chat_line,
                } = &message.payload
                {
                    let _ = event_tx.send(LanEvent::Chat(
                        message.sender.name.clone(),
                        sender_ip,
                        chat_line.clone(),
                        *chat_type,
                    ));
                }
            }
            LanMessageType::Inactive => {
                let _ = event_tx.send(LanEvent::PlayerInactive(sender_ip));
            }
        }

        Ok(())
    }

    /// Get the next event (non-blocking)
    pub async fn poll_event(&mut self) -> Option<LanEvent> {
        match self.event_rx.try_recv() {
            Ok(event) => Some(event),
            Err(broadcast::error::TryRecvError::Empty) => None,
            Err(broadcast::error::TryRecvError::Lagged(_)) => {
                warn!("Event receiver lagged, some events may have been missed");
                None
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                error!("Event channel closed");
                None
            }
        }
    }

    /// Update the LAN API and return any events
    pub async fn update(&mut self) -> NetworkResult<Option<LanEvent>> {
        // Update discovery service
        self.discovery.update().await?;

        // Update lobby
        {
            let mut lobby = self.lobby.write().await;
            lobby.update().await?;
        }

        // Update chat
        {
            let mut chat = self.chat.write().await;
            chat.update().await?;
        }

        // Return any pending events
        Ok(self.poll_event().await)
    }

    /// Set whether the application is active
    pub async fn set_is_active(&self, is_active: bool) {
        *self.is_active.write().await = is_active;
        debug!("Application active state changed to: {}", is_active);
    }

    /// Request location discovery (find all players and games)
    pub async fn request_locations(&self) -> NetworkResult<()> {
        info!("Requesting location discovery");
        self.discovery.request_locations().await?;
        let lobby = self.lobby.read().await;
        lobby.request_locations().await
    }

    /// Request to join a specific game
    pub async fn request_game_join(
        &self,
        game: &LanGameInfo,
        ip: Option<IpAddr>,
    ) -> NetworkResult<()> {
        info!("Requesting to join game: {}", game.name);

        // Check if we're already busy with another action
        {
            let pending = *self.pending_action.read().await;
            if pending != PendingAction::None {
                return Err(NetworkError::generic(
                    "Another action is already in progress".to_string(),
                ));
            }
        }

        // Set pending action
        {
            *self.pending_action.write().await = PendingAction::Join;
            *self.action_expiration.write().await =
                Some(NetworkInstant::now() + self.config.action_timeout);
        }

        let lobby = self.lobby.write().await;
        lobby.request_join(game, ip).await
    }

    /// Request direct connect to an IP address
    pub async fn request_game_join_direct_connect(&self, ip_address: IpAddr) -> NetworkResult<()> {
        info!("Requesting direct connect to: {}", ip_address);

        // Check if we're already busy
        {
            let pending = *self.pending_action.read().await;
            if pending != PendingAction::None {
                return Err(NetworkError::generic(
                    "Another action is already in progress".to_string(),
                ));
            }
        }

        // Set pending action
        {
            *self.pending_action.write().await = PendingAction::JoinDirectConnect;
            *self.action_expiration.write().await =
                Some(NetworkInstant::now() + self.config.action_timeout);
        }

        let lobby = self.lobby.write().await;
        lobby.request_direct_connect(ip_address).await
    }

    /// Request to leave current game
    pub async fn request_game_leave(&self) -> NetworkResult<()> {
        info!("Requesting to leave game");
        let lobby = self.lobby.write().await;
        lobby.request_leave().await
    }

    /// Update acceptance of current game options
    pub async fn request_accept(&self, accepted: bool) -> NetworkResult<()> {
        info!("Requesting accept status: {}", accepted);
        let lobby = self.lobby.read().await;
        lobby.request_accept(accepted).await
    }

    /// Convenience helper matching the legacy API: mark that we have the map.
    pub async fn request_has_map(&self) -> NetworkResult<()> {
        self.request_map_status(true).await
    }

    /// Announce local map availability status
    pub async fn request_map_status(&self, has_map: bool) -> NetworkResult<()> {
        info!("Announcing map status: {}", has_map);
        let lobby = self.lobby.read().await;
        lobby.request_map_status(has_map).await
    }

    /// Send a chat message
    pub async fn request_chat(&self, message: String, chat_type: ChatType) -> NetworkResult<()> {
        if message.len() > self.config.max_chat_length {
            return Err(NetworkError::invalid_command(format!(
                "Chat message too long: {} > {}",
                message.len(),
                self.config.max_chat_length
            )));
        }

        let lobby = self.lobby.read().await;
        lobby.request_chat(chat_type, message).await
    }

    /// Request to start the game
    pub async fn request_game_start(&self) -> NetworkResult<()> {
        info!("Requesting game start");
        let lobby = self.lobby.read().await;
        lobby.request_game_start().await
    }

    /// Request game start with countdown timer
    pub async fn request_game_start_timer(&self, seconds: u32) -> NetworkResult<()> {
        info!("Requesting game start timer: {} seconds", seconds);
        let lobby = self.lobby.read().await;
        lobby.request_game_start_timer(seconds).await
    }

    /// Request to update game options
    pub async fn request_game_options(
        &self,
        game_options: GameOptions,
        is_public: bool,
        ip: Option<IpAddr>,
    ) -> NetworkResult<()> {
        info!("Requesting game options update");
        let lobby = self.lobby.read().await;
        lobby
            .request_game_options(game_options, is_public, ip)
            .await
    }

    /// Request to create a new game
    pub async fn request_game_create(
        &self,
        game_name: String,
        is_direct_connect: bool,
    ) -> NetworkResult<()> {
        if game_name.len() > self.config.max_game_name_length {
            return Err(NetworkError::invalid_command(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                self.config.max_game_name_length
            )));
        }

        info!("Requesting game creation: {}", game_name);

        // Check if we're already busy
        {
            let pending = *self.pending_action.read().await;
            if pending != PendingAction::None {
                return Err(NetworkError::generic(
                    "Another action is already in progress".to_string(),
                ));
            }
        }

        // Set pending action
        {
            *self.pending_action.write().await = PendingAction::CreateGame;
            *self.action_expiration.write().await =
                Some(NetworkInstant::now() + self.config.action_timeout);
        }

        // Set as host
        *self.is_host.write().await = true;

        let lobby = self.lobby.write().await;
        lobby
            .request_create_game(game_name, is_direct_connect)
            .await
    }

    /// Request to announce current game
    pub async fn request_game_announce(&self) -> NetworkResult<()> {
        info!("Requesting game announcement");
        let lobby = self.lobby.read().await;
        lobby.request_announce().await
    }

    /// Request to change player name
    pub async fn request_set_name(&self, new_name: String) -> NetworkResult<()> {
        if new_name.len() > self.config.max_game_name_length {
            return Err(NetworkError::invalid_command(format!(
                "Name too long: {} > {}",
                new_name.len(),
                self.config.max_game_name_length
            )));
        }

        info!("Requesting name change to: {}", new_name);
        let lobby = self.lobby.read().await;
        lobby.request_name_change(new_name).await
    }

    /// Request to leave the lobby
    pub async fn request_lobby_leave(&self, forced: bool) -> NetworkResult<()> {
        info!("Requesting lobby leave (forced: {})", forced);
        {
            let mut pending = self.pending_action.write().await;
            *pending = PendingAction::Leave;
        }
        *self.action_expiration.write().await =
            Some(NetworkInstant::now() + self.config.action_timeout);

        let lobby = self.lobby.write().await;
        let result = lobby.request_lobby_leave(forced).await;

        *self.pending_action.write().await = PendingAction::None;
        *self.action_expiration.write().await = None;

        result
    }

    /// Reset game start timer
    pub async fn reset_game_start_timer(&self) -> NetworkResult<()> {
        info!("Resetting game start timer");
        let lobby = self.lobby.read().await;
        lobby.reset_game_start_timer().await
    }

    /// Notify peers that we became inactive/active
    pub async fn set_inactive(&self, inactive: bool) -> NetworkResult<()> {
        let lobby = self.lobby.read().await;
        lobby.set_inactive(inactive).await
    }

    /// Look up a game by name
    pub async fn lookup_game(&self, game_name: &str) -> Option<LanGameInfo> {
        let games = self.games.read().await;
        games.get(game_name).cloned()
    }

    /// Look up a game by list offset
    pub async fn lookup_game_by_offset(&self, offset: usize) -> Option<LanGameInfo> {
        let games = self.games.read().await;
        games.values().nth(offset).cloned()
    }

    /// Set local IP address
    pub async fn set_local_ip(&self, local_ip: IpAddr) -> NetworkResult<bool> {
        info!("Setting local IP to: {}", local_ip);
        *self.local_ip.write().await = Some(local_ip);

        self.discovery.set_local_ip(local_ip).await;
        {
            let lobby = self.lobby.read().await;
            lobby.set_local_ip(local_ip).await;
        }

        Ok(true)
    }

    /// Get local IP address
    pub async fn get_local_ip(&self) -> Option<IpAddr> {
        *self.local_ip.read().await
    }

    /// Check if we are the host
    pub async fn am_i_host(&self) -> bool {
        *self.is_host.read().await
    }

    /// Get our player name
    pub async fn get_my_name(&self) -> String {
        self.config.player_name.clone()
    }

    /// Get current game we're in
    pub async fn get_my_game(&self) -> Option<LanGameInfo> {
        self.current_game.read().await.clone()
    }

    /// Get all discovered games
    pub async fn get_game_list(&self) -> Vec<LanGameInfo> {
        let games = self.games.read().await;
        games.values().cloned().collect()
    }

    /// Get all lobby players
    pub async fn get_player_list(&self) -> Vec<LanPlayer> {
        let players = self.lobby_players.read().await;
        players.values().cloned().collect()
    }

    /// Shutdown the LAN API
    pub async fn shutdown(&self) -> NetworkResult<()> {
        info!("Shutting down LAN API");

        // Send shutdown signal
        let _ = self.bridge_tx.send(LanBridgeEvent::Shutdown);

        // Wait for background task to complete
        if let Some(handle) = self.bg_task.write().await.take() {
            let _ = handle.await;
        }

        if let Some(handle) = self.nat_task.write().await.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Shutdown components
        self.discovery.shutdown().await?;

        {
            let mut lobby = self.lobby.write().await;
            lobby.shutdown().await?;
        }

        {
            let mut chat = self.chat.write().await;
            chat.shutdown().await?;
        }

        info!("LAN API shut down successfully");
        Ok(())
    }
}

impl LanApi {
    async fn apply_nat_binding(
        discovery: Arc<GameDiscovery>,
        lobby: Arc<RwLock<LanLobby>>,
        binding: Option<NatBinding>,
    ) -> NetworkResult<()> {
        let endpoint = binding.as_ref().map(|entry| entry.address);

        if let Some(ref found) = binding {
            info!(
                address = %found.address,
                server = %found.server,
                "Applying NAT public endpoint to LAN services"
            );
        } else {
            debug!("Clearing NAT public endpoint for LAN services");
        }

        discovery.set_public_endpoint(endpoint).await;

        {
            let mut lobby_guard = lobby.write().await;
            lobby_guard.set_public_endpoint(endpoint).await;
        }

        Ok(())
    }
}

impl Drop for LanApi {
    fn drop(&mut self) {
        // Attempt clean shutdown. Ignore errors when runtime is already gone.
        let _ = self.bridge_tx.send(LanBridgeEvent::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NetworkResult;
    use crate::observability::{
        initialize_telemetry, telemetry, HealthStatus, ObservabilityConfig,
    };
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;
    use tokio::sync::{watch, OnceCell};
    use tokio::time::sleep;

    fn next_port() -> u16 {
        std::net::UdpSocket::bind("127.0.0.1:0")
            .expect("allocate ephemeral port")
            .local_addr()
            .expect("socket addr")
            .port()
    }

    fn integration_config(name: &str, port: u16) -> LanConfig {
        LanConfig {
            player_name: name.to_string(),
            login_name: format!("{}_login", name),
            host_name: format!("{}_host", name),
            base_port: port,
            broadcast_addr: Ipv4Addr::new(127, 0, 0, 1),
            mdns_service: format!("{}_service._udp.local", name.to_lowercase()),
            action_timeout: Duration::from_secs(4),
            resend_interval: Duration::from_secs(1),
            max_players: 4,
            enable_mdns: false,
            enable_broadcast: true,
            enable_host_migration: false,
            max_chat_length: 128,
            max_game_name_length: 24,
        }
    }

    async fn wait_for_event<F>(
        api: &mut LanApi,
        limit: Duration,
        label: &str,
        mut predicate: F,
    ) -> NetworkResult<()>
    where
        F: FnMut(&LanEvent) -> bool,
    {
        let step = Duration::from_millis(20);
        let mut elapsed = Duration::ZERO;

        while elapsed <= limit {
            if let Some(event) = pump(api).await? {
                if predicate(&event) {
                    return Ok(());
                }
            }

            tokio::time::advance(step).await;
            elapsed += step;
        }

        Err(NetworkError::generic(format!(
            "timed out waiting for {}",
            label
        )))
    }

    async fn ensure_telemetry() -> NetworkResult<()> {
        static TELEMETRY_INIT: OnceCell<()> = OnceCell::const_new();

        if telemetry().is_some() {
            return Ok(());
        }

        TELEMETRY_INIT
            .get_or_try_init(|| {
                Box::pin(async {
                    if telemetry().is_some() {
                        return Ok(());
                    }

                    let mut config = ObservabilityConfig::default();
                    config.enable_metrics = false;
                    config.enable_tracing = false;
                    config.enable_console = false;
                    match initialize_telemetry(config).await {
                        Ok(_) => Ok(()),
                        Err(NetworkError::Generic { message })
                            if message == "Telemetry already initialized" =>
                        {
                            Ok(())
                        }
                        Err(err) => Err(err),
                    }
                })
            })
            .await
            .map(|_| ())
    }

    async fn pump(api: &mut LanApi) -> NetworkResult<Option<LanEvent>> {
        api.update().await
    }

    async fn drive_until<F>(
        host: &mut LanApi,
        client: &mut LanApi,
        timeout: Duration,
        mut predicate: F,
    ) -> NetworkResult<bool>
    where
        F: FnMut(bool, &LanEvent) -> bool,
    {
        let deadline = NetworkInstant::now() + timeout;
        while NetworkInstant::now() < deadline {
            if let Some(event) = pump(host).await? {
                if predicate(true, &event) {
                    return Ok(true);
                }
            }

            if let Some(event) = pump(client).await? {
                if predicate(false, &event) {
                    return Ok(true);
                }
            }

            sleep(Duration::from_millis(25)).await;
        }
        Ok(false)
    }

    #[tokio::test]
    async fn nat_bindings_update_discovery_and_lobby() -> NetworkResult<()> {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let host_port = next_port();
        let mut host = LanApi::new(integration_config("HostNat", host_port)).await?;
        host.init().await?;
        host.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await?;

        host.request_game_create("NatShowcase".into(), false)
            .await?;

        let deadline = NetworkInstant::now() + Duration::from_secs(2);
        let mut created = false;
        while NetworkInstant::now() < deadline {
            if let Some(event) = pump(&mut host).await? {
                if matches!(event, LanEvent::GameCreate(LanResult::Ok)) {
                    created = true;
                    break;
                }
            }
            sleep(Duration::from_millis(20)).await;
        }
        assert!(created, "host should acknowledge game creation");

        let (binding_tx, binding_rx) = watch::channel(None);
        host.attach_nat_updates(binding_rx).await?;

        let binding = NatBinding {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10)), 62000),
            server: "unit-test".into(),
            round_trip_time: Duration::from_millis(42),
            obtained_at: NetworkInstant::now(),
        };

        binding_tx
            .send(Some(binding.clone()))
            .expect("send nat binding");
        sleep(Duration::from_millis(100)).await;

        let discovery_endpoint = host.discovery.public_endpoint().await;
        assert_eq!(discovery_endpoint, Some(binding.address));

        let snapshot = {
            let lobby = host.lobby.read().await;
            lobby.current_game_snapshot().await
        }
        .expect("hosted game snapshot");
        assert_eq!(snapshot.public_host, Some(binding.address.ip()));
        assert_eq!(snapshot.public_port, Some(binding.address.port()));

        binding_tx.send(None).expect("clear binding");
        sleep(Duration::from_millis(100)).await;
        pump(&mut host).await?;

        let cleared = host.discovery.public_endpoint().await;
        assert!(cleared.is_none());

        host.shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn discovery_join_leave_direct_connect_flow() -> NetworkResult<()> {
        ensure_telemetry().await?;

        let host_port = next_port();
        let client_port = next_port();

        let mut host = LanApi::new(integration_config("Host", host_port)).await?;
        host.init().await?;
        host.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await?;

        let mut client = LanApi::new(integration_config("Client", client_port)).await?;
        client.init().await?;
        client.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await?;

        host.request_game_create("TestGame".to_string(), false)
            .await?;
        client.request_locations().await?;

        let mut discovered: Option<LanGameInfo> = None;
        drive_until(
            &mut host,
            &mut client,
            Duration::from_secs(5),
            |is_host, event| {
                if !is_host {
                    if let LanEvent::GameList(list) = event {
                        if let Some(game) = list.iter().find(|g| g.name == "TestGame") {
                            discovered = Some(game.clone());
                            return true;
                        }
                    }
                }
                false
            },
        )
        .await?;

        if discovered.is_none() {
            discovered = host
                .get_game_list()
                .await
                .into_iter()
                .find(|g| g.name == "TestGame");
        }

        let game_info = discovered.expect("client discovered host game");
        client.request_game_join(&game_info, None).await?;

        drive_until(
            &mut host,
            &mut client,
            Duration::from_secs(5),
            |is_host, event| match (is_host, event) {
                (true, LanEvent::PlayerJoin(name, _)) if name == "Client" => true,
                (false, LanEvent::GameJoin(LanResult::Ok, Some(game)))
                    if game.name == "TestGame" =>
                {
                    true
                }
                _ => false,
            },
        )
        .await?;

        client.request_accept(true).await?;
        drive_until(&mut host, &mut client, Duration::from_secs(3), |is_host, event| {
            matches!(event, LanEvent::AcceptStatus(ip, true) if is_host && *ip == IpAddr::V4(Ipv4Addr::LOCALHOST))
        })
        .await?;

        client.request_game_leave().await?;
        drive_until(&mut host, &mut client, Duration::from_secs(3), |is_host, event| {
            matches!(event, LanEvent::PlayerLeave(name) if is_host && name == "Client")
        })
        .await?;

        client
            .request_game_join_direct_connect(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .await?;

        drive_until(
            &mut host,
            &mut client,
            Duration::from_secs(5),
            |is_host, event| match (is_host, event) {
                (true, LanEvent::PlayerJoin(name, _)) if name == "Client" => true,
                (false, LanEvent::GameJoin(LanResult::Ok, Some(game)))
                    if game.name == "TestGame" =>
                {
                    true
                }
                _ => false,
            },
        )
        .await?;

        client.request_game_leave().await?;
        drive_until(&mut host, &mut client, Duration::from_secs(3), |is_host, event| {
            matches!(event, LanEvent::PlayerLeave(name) if is_host && name == "Client")
        })
        .await?;

        let telemetry = telemetry().expect("telemetry initialized");
        let report = telemetry.generate_health_report().await;
        assert!(matches!(report.status, HealthStatus::Healthy));

        client.shutdown().await?;
        host.shutdown().await?;

        Ok(())
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn lan_api_chat_emits_events() -> NetworkResult<()> {
        ensure_telemetry().await?;
        let host_port = next_port();
        let mut host = LanApi::new(integration_config("ChatHost", host_port)).await?;
        host.init().await?;
        host.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await?;

        host.request_game_create("ChatGame".to_string(), false)
            .await?;

        wait_for_event(
            &mut host,
            Duration::from_secs(2),
            "game creation",
            |event| matches!(event, LanEvent::GameCreate(LanResult::Ok)),
        )
        .await?;

        host.request_chat("hello world".to_string(), ChatType::Normal)
            .await?;

        wait_for_event(&mut host, Duration::from_secs(2), "chat echo", |event| {
            if let LanEvent::Chat(name, ip, message, chat_type) = event {
                assert_eq!(name, "ChatHost");
                assert_eq!(*ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
                assert_eq!(message, "hello world");
                assert_eq!(*chat_type, ChatType::Normal);
                true
            } else {
                false
            }
        })
        .await?;

        host.shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    #[ignore] // Timer uses NetworkInstant which doesn't respond to tokio's virtual time
    async fn lan_api_game_start_timer_events() -> NetworkResult<()> {
        ensure_telemetry().await?;
        let host_port = next_port();
        let mut host = LanApi::new(integration_config("TimerHost", host_port)).await?;
        host.init().await?;
        host.set_local_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)).await?;

        host.request_game_create("TimerGame".to_string(), false)
            .await?;

        wait_for_event(
            &mut host,
            Duration::from_secs(2),
            "game creation",
            |event| matches!(event, LanEvent::GameCreate(LanResult::Ok)),
        )
        .await?;

        host.request_game_start_timer(1).await?;

        // Advance virtual time to trigger the countdown instantly.
        tokio::time::advance(Duration::from_secs(1)).await;

        let mut saw_tick = false;
        wait_for_event(
            &mut host,
            Duration::from_secs(1),
            "game start sequence",
            |event| match event {
                LanEvent::GameStartTimer(_) => {
                    saw_tick = true;
                    false
                }
                LanEvent::GameStart => saw_tick,
                _ => false,
            },
        )
        .await?;
        assert!(saw_tick, "expected at least one countdown tick event");

        host.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_lan_api_creation() {
        let config = LanConfig::default();
        let lan_api = LanApi::new(config).await.unwrap();

        assert_eq!(lan_api.get_my_name().await, "Player");
        assert!(!lan_api.am_i_host().await);
        assert!(lan_api.get_local_ip().await.is_none());
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = LanConfig::default();
        config.player_name = "ThisNameIsTooLongForTheLimit".to_string();
        config.max_game_name_length = 10;

        let result = LanApi::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_ip_setting() {
        let config = LanConfig::default();
        let lan_api = LanApi::new(config).await.unwrap();

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let success = lan_api.set_local_ip(test_ip).await.unwrap();

        assert!(success);
        assert_eq!(lan_api.get_local_ip().await, Some(test_ip));
    }
}
