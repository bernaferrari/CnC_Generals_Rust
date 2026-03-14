#![allow(dead_code, unused_variables)]
/*
** Command & Conquer Generals Zero Hour(tm)
** Copyright 2025 Electronic Arts Inc.
**
** This program is free software: you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation, either version 3 of the License, or
** (at your option) any later version.
**
** This program is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
**
** You should have received a copy of the GNU General Public License
** along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

//! Network System Integration
//!
//! This module provides integration between the Main game module and the
//! GameNetwork module, exposing a simplified API that matches what the
//! network_demo and other Main binaries expect.

use bincode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, OnceLock, RwLock as StdRwLock};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, RwLock};

use game_network as real_net;
use game_network::lan_api::{GameOptions, LanApi, LanConfig, LanEvent, LanGameInfo};
use game_network::NetworkInterface as RealNetworkInterface;
use gamelogic::commands::command::CommandType;
use real_net::commands::{CommandParameter, GameCommandData};
use real_net::{NetCommand, NetCommandType, TransportProtocol};

/// Network error type (stub)
#[derive(Debug, Clone)]
pub enum NetworkError {
    ConnectionFailed,
    Timeout,
    InvalidData,
    Other(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::ConnectionFailed => write!(f, "Connection failed"),
            NetworkError::Timeout => write!(f, "Network timeout"),
            NetworkError::InvalidData => write!(f, "Invalid network data"),
            NetworkError::Other(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<real_net::NetworkError> for NetworkError {
    fn from(err: real_net::NetworkError) -> Self {
        NetworkError::Other(err.to_string())
    }
}

/// Network result type
pub type NetworkResult<T> = Result<T, NetworkError>;

/// Network configuration for Main module integration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub port: u16,
    pub enable_lan: bool,
    pub max_players: u8,
    pub timeout_ms: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 8088,
            enable_lan: true,
            max_players: 8,
            timeout_ms: 5000,
        }
    }
}

/// Player information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: u32,
    pub name: String,
    pub address: SocketAddr,
}

impl PlayerInfo {
    pub fn new(id: u32, name: String, address: SocketAddr) -> Self {
        Self { id, name, address }
    }
}

/// Game information for lobby
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub name: String,
    pub map_name: String,
    pub current_players: u8,
    pub max_players: u8,
    pub host_address: SocketAddr,
}

/// Chat message types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ChatType {
    All,
    Team,
    Private,
}

/// Unit command types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnitCommandType {
    Move,
    Attack,
    Stop,
    Guard,
    Build,
}

/// Command targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandTarget {
    Position(glam::Vec2),
    Unit(u32),
    None,
}

/// Unit command structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitCommand {
    pub command_type: UnitCommandType,
    pub unit_ids: Vec<u32>,
    pub target: Option<CommandTarget>,
    pub parameters: Vec<u8>,
}

/// Network state enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NetworkState {
    Disconnected,
    Connecting,
    Connected,
    InLobby,
    InGame,
    Disconnecting,
}

/// Synchronization state
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SyncState {
    Synchronized,
    Waiting,
    OutOfSync,
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStatistics {
    pub state: NetworkState,
    pub connected_players: u32,
    pub current_frame: u32,
    pub sync_state: SyncState,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub uptime: Duration,
}

impl Default for NetworkStatistics {
    fn default() -> Self {
        Self {
            state: NetworkState::Disconnected,
            connected_players: 0,
            current_frame: 0,
            sync_state: SyncState::Synchronized,
            bytes_sent: 0,
            bytes_received: 0,
            uptime: Duration::from_secs(0),
        }
    }
}

/// Lobby callbacks for events
#[derive(Default)]
pub struct LobbyCallbacks {
    pub on_player_joined: Option<Box<dyn Fn(PlayerInfo) + Send + Sync>>,
    pub on_player_left: Option<Box<dyn Fn(u32) + Send + Sync>>,
    pub on_game_started: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_game_found: Option<Box<dyn Fn(GameInfo) + Send + Sync>>,
    pub on_chat_received: Option<Box<dyn Fn(u32, String, ChatType) + Send + Sync>>,
}

/// Main network interface that wraps the GameNetwork module
pub struct NetworkInterface {
    inner: Arc<RwLock<RealNetworkInterface>>,
    config: RwLock<NetworkConfig>,
    local_player: RwLock<Option<PlayerInfo>>,
    player_teams: RwLock<HashMap<u8, u8>>,
    available_games: RwLock<Vec<GameInfo>>,
    statistics: RwLock<NetworkStatistics>,
    callbacks: RwLock<LobbyCallbacks>,
    lan_api: Mutex<Option<LanApi>>,
}

fn active_interface_slot() -> &'static StdRwLock<Option<Arc<RwLock<NetworkInterface>>>> {
    static SLOT: OnceLock<StdRwLock<Option<Arc<RwLock<NetworkInterface>>>>> = OnceLock::new();
    SLOT.get_or_init(|| StdRwLock::new(None))
}

fn set_active_network_interface(interface: Arc<RwLock<NetworkInterface>>) {
    if let Ok(mut slot) = active_interface_slot().write() {
        *slot = Some(interface);
    }
}

pub fn clear_active_network_interface() {
    if let Ok(mut slot) = active_interface_slot().write() {
        *slot = None;
    }
}

pub fn has_active_network_interface() -> bool {
    active_interface_slot()
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().map(|_| true))
        .unwrap_or(false)
}

pub fn active_session_frame_data_ready() -> Option<bool> {
    let interface = active_interface_slot()
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().cloned())?;
    let handle = Handle::try_current().ok()?;
    Some(handle.block_on(async {
        let net_guard = interface.read().await;
        net_guard.is_ready_for_commands().await
    }))
}

impl NetworkInterface {
    /// Create or return the shared LAN API used for discovery/hosting.
    async fn ensure_lan_api(&self) -> NetworkResult<tokio::sync::MutexGuard<'_, Option<LanApi>>> {
        let mut lan_guard = self.lan_api.lock().await;
        if lan_guard.is_none() {
            let cfg = self.config.read().await.clone();
            let player_name = self
                .local_player
                .read()
                .await
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Player".to_string());
            let mut lan_config = LanConfig::default();
            lan_config.player_name = player_name.clone();
            lan_config.login_name = player_name.clone();
            lan_config.host_name = player_name;
            lan_config.base_port = cfg.port;
            lan_config.broadcast_addr = Ipv4Addr::BROADCAST;
            lan_config.resend_interval = Duration::from_millis(cfg.timeout_ms.max(500));
            lan_config.max_players = cfg.max_players;
            lan_config.enable_broadcast = cfg.enable_lan;
            lan_config.enable_mdns = cfg.enable_lan;
            let mut api = LanApi::new(lan_config).await.map_err(NetworkError::from)?;
            api.init().await.map_err(NetworkError::from)?;
            *lan_guard = Some(api);
        }
        Ok(lan_guard)
    }

    /// Create a new network interface
    pub fn new(config: NetworkConfig) -> NetworkResult<Self> {
        let handle = Handle::try_current()
            .map_err(|e| NetworkError::Other(format!("No async runtime for network init: {e}")))?;

        let real_config = real_net::NetworkConfig {
            player_id: 0,
            max_frames_ahead: real_net::config::MAX_FRAMES_AHEAD,
            min_runahead: real_net::config::MIN_RUNAHEAD,
            max_run_ahead: real_net::config::MAX_FRAMES_AHEAD / 2,
            target_frame_rate: real_net::config::TARGET_FPS,
            enable_compression: true,
            enable_encryption: true,
            debug_mode: cfg!(debug_assertions),
            nat: real_net::NatConfig::default(),
            firewall: real_net::FirewallConfig::default(),
        };

        let inner = handle
            .block_on(RealNetworkInterface::new(real_config))
            .map_err(NetworkError::from)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            config: RwLock::new(config),
            local_player: RwLock::new(None),
            player_teams: RwLock::new(HashMap::from([(0, 0)])),
            available_games: RwLock::new(Vec::new()),
            statistics: RwLock::new(NetworkStatistics::default()),
            callbacks: RwLock::new(LobbyCallbacks::default()),
            lan_api: Mutex::new(None),
        })
    }

    /// Initialize the network interface
    pub async fn initialize(&self, config: NetworkConfig) -> NetworkResult<()> {
        log::info!("Initializing network interface with config: {:?}", config);
        // Bind to the requested port and refresh NAT mappings.
        {
            let mut guard = self.inner.write().await;
            guard
                .init_local(config.port)
                .await
                .map_err(NetworkError::from)?;
            guard
                .set_local_address(Ipv4Addr::UNSPECIFIED, config.port)
                .await
                .map_err(NetworkError::from)?;
        }

        // Reset network frame state for a new session.
        {
            let guard = self.inner.read().await;
            guard.start_game().await.map_err(NetworkError::from)?;
        }
        {
            let mut stored_config = self.config.write().await;
            *stored_config = config;
        }
        Ok(())
    }

    /// Set the local player
    pub async fn set_local_player(&self, player: PlayerInfo) -> NetworkResult<()> {
        log::info!("Setting local player: {:?}", player);
        {
            let mut local_player = self.local_player.write().await;
            *local_player = Some(player);
        }
        Ok(())
    }

    /// Assign a team id to a player for team-chat filtering (same team = allied).
    ///
    /// This only affects how the chat `player_mask` is computed; it does not change game state.
    pub async fn set_player_team(&self, player_id: u8, team: u8) {
        let mut guard = self.player_teams.write().await;
        guard.insert(player_id, team);
    }

    async fn local_player_id(&self) -> u8 {
        let guard = self.inner.read().await;
        guard.local_player_id()
    }

    async fn team_chat_mask(&self) -> u8 {
        let max_players = self.config.read().await.max_players.min(8);
        let local_id = self.local_player_id().await;
        let local_team = self
            .player_teams
            .read()
            .await
            .get(&local_id)
            .copied()
            .unwrap_or(local_id);

        let teams = self.player_teams.read().await;
        let mut mask = 0u8;
        for pid in 0..max_players {
            let team = teams.get(&pid).copied().unwrap_or(pid);
            if team == local_team {
                mask |= 1u8 << pid;
            }
        }
        mask
    }

    /// Set lobby callbacks
    pub async fn set_lobby_callbacks(&self, callbacks: LobbyCallbacks) -> NetworkResult<()> {
        log::info!("Setting lobby callbacks");
        {
            let mut stored_callbacks = self.callbacks.write().await;
            *stored_callbacks = callbacks;
        }
        Ok(())
    }

    /// Start the network interface
    pub async fn start(&self) -> NetworkResult<()> {
        let port = {
            let cfg = self.config.read().await;
            cfg.port
        };
        log::info!("Starting network interface on port {}", port);
        let mut guard = self.inner.write().await;
        guard.init_local(port).await.map_err(NetworkError::from)
    }

    /// Update network state
    pub async fn update(&self) -> NetworkResult<()> {
        let guard = self.inner.read().await;
        guard.update_concurrent().await.map_err(NetworkError::from)
    }

    /// Host a new game
    pub async fn host_game(
        &self,
        name: String,
        map: String,
        max_players: u8,
        _password: Option<String>,
    ) -> NetworkResult<()> {
        log::info!(
            "Hosting game: '{}' on map '{}' with {} max players",
            name,
            map,
            max_players
        );
        // Advertise self as host on LAN and refresh player list (single-player host for now).
        let port = self.port().await;
        {
            let guard = self.inner.read().await;
            guard.set_player_name(0, name.clone()).await;
            guard.start_game().await.map_err(NetworkError::from)?;
            guard
                .parse_user_list(&[real_net::PlayerEndpoint {
                    player_id: 0,
                    address: SocketAddr::from(([127, 0, 0, 1], port)),
                    display_name: Some(name.clone()),
                    protocol: TransportProtocol::Udp,
                }])
                .await
                .map_err(NetworkError::from)?;
        }

        // Broadcast lobby via LAN API for discovery.
        {
            let mut lan_guard = self.ensure_lan_api().await?;
            if let Some(api) = lan_guard.as_mut() {
                api.request_set_name(name.clone())
                    .await
                    .map_err(NetworkError::from)?;
                api.request_game_create(name.clone(), false)
                    .await
                    .map_err(NetworkError::from)?;
                let mut opts = GameOptions::default();
                opts.map_name = map.clone();
                api.request_game_options(opts, true, None)
                    .await
                    .map_err(NetworkError::from)?;
                api.request_game_announce()
                    .await
                    .map_err(NetworkError::from)?;
            }
        }

        Ok(())
    }

    /// Join an existing game
    pub async fn join_game(&self, game_name: &str, password: Option<String>) -> NetworkResult<()> {
        log::info!(
            "Joining game: '{}' with password: {:?}",
            game_name,
            password.is_some()
        );
        // Resolve target game via LAN discovery.
        let mut target: Option<SocketAddr> = None;
        {
            let mut lan_guard = self.ensure_lan_api().await?;
            if let Some(api) = lan_guard.as_mut() {
                api.update().await.map_err(NetworkError::from)?;
                if let Some(info) = api.lookup_game(game_name).await {
                    target = Some(
                        info.public_endpoint()
                            .unwrap_or(SocketAddr::new(info.host_ip, info.port)),
                    );
                    api.request_game_join(&info, None)
                        .await
                        .map_err(NetworkError::from)?;
                }
            }
        }

        let host_addr = target.unwrap_or(SocketAddr::from(([127, 0, 0, 1], self.port().await)));
        {
            let guard = self.inner.read().await;
            guard
                .connect_player(1, host_addr)
                .await
                .map_err(NetworkError::from)?;
            guard.start_game().await.map_err(NetworkError::from)?;
        }
        Ok(())
    }

    /// Start the game (transition from lobby to game)
    pub async fn start_game(&self) -> NetworkResult<()> {
        log::info!("Starting game (resetting network sync)");
        let guard = self.inner.read().await;
        guard.start_game().await.map_err(NetworkError::from)
    }

    /// Refresh available games list
    pub async fn refresh_games(&self) -> NetworkResult<()> {
        log::info!("Refreshing available games");
        let mut discovered = Vec::new();
        {
            let mut lan_guard = self.ensure_lan_api().await?;
            if let Some(api) = lan_guard.as_mut() {
                api.update().await.map_err(NetworkError::from)?;
                while let Some(event) = api.poll_event().await {
                    if let LanEvent::GameList(list) = event {
                        discovered = list;
                    }
                }
                if discovered.is_empty() {
                    discovered = api.get_game_list().await;
                }
            }
        }

        let mut games = self.available_games.write().await;
        games.clear();
        if discovered.is_empty() {
            // Fallback to local entry to keep UI populated when no LAN games are present.
            let cfg = self.config.read().await.clone();
            games.push(GameInfo {
                name: "LocalHost".to_string(),
                map_name: "DemoMap".to_string(),
                current_players: 1,
                max_players: cfg.max_players,
                host_address: SocketAddr::from(([127, 0, 0, 1], cfg.port)),
            });
        } else {
            for info in discovered {
                games.push(Self::convert_lan_game(info));
            }
        }
        Ok(())
    }

    fn convert_lan_game(info: LanGameInfo) -> GameInfo {
        let endpoint = info
            .public_endpoint()
            .unwrap_or(SocketAddr::new(info.host_ip, info.port));
        GameInfo {
            name: info.name,
            map_name: if info.options.map_name.is_empty() {
                "Unknown".to_string()
            } else {
                info.options.map_name
            },
            current_players: info.player_count,
            max_players: info.max_players,
            host_address: endpoint,
        }
    }

    /// Get available games
    pub async fn get_available_games(&self) -> Vec<GameInfo> {
        let games = self.available_games.read().await;
        games.clone()
    }

    /// Send chat message
    pub async fn send_chat(
        &self,
        message: String,
        chat_type: ChatType,
        target_player: Option<u32>,
    ) -> NetworkResult<()> {
        log::info!(
            "Sending chat ({:?}): '{}' to player {:?}",
            chat_type,
            message,
            target_player
        );
        let mask = match chat_type {
            ChatType::All => 0,
            ChatType::Team => self.team_chat_mask().await,
            ChatType::Private => {
                if let Some(pid) = target_player {
                    if pid < 8 {
                        1u8 << (pid as u8)
                    } else {
                        return Err(NetworkError::Other(
                            "Private chat target out of range".into(),
                        ));
                    }
                } else {
                    return Err(NetworkError::Other(
                        "Private chat requires a target player".into(),
                    ));
                }
            }
        };
        let guard = self.inner.read().await;
        guard
            .send_chat_message(message, mask)
            .await
            .map_err(NetworkError::from)
    }

    /// Send unit command
    pub async fn send_unit_command(&self, command: UnitCommand) -> NetworkResult<()> {
        log::info!(
            "Sending unit command: {:?} for {} units",
            command.command_type,
            command.unit_ids.len()
        );
        let net_command = NetCommand::game_command(0, 0, to_game_command_data(&command));
        let guard = self.inner.read().await;
        guard
            .send_command(net_command)
            .await
            .map_err(NetworkError::from)
    }

    /// Check if ready for commands
    pub async fn is_ready_for_commands(&self) -> bool {
        let guard = self.inner.read().await;
        guard.is_frame_data_ready().await
    }

    /// Get network statistics
    pub async fn get_statistics(&self) -> NetworkStatistics {
        let (connected_players, bytes_sent, bytes_received, current_frame) = {
            let guard = self.inner.read().await;
            let load_progress_rx = guard.subscribe_load_progress();
            let connected_players = load_progress_rx.borrow().len() as u32;

            let rx_rate = guard.incoming_bytes_per_second().await;
            let tx_rate = guard.outgoing_bytes_per_second().await;
            let frame = guard.execution_frame();
            (connected_players, tx_rate as u64, rx_rate as u64, frame)
        };

        let mut stats = self.statistics.write().await;
        *stats = NetworkStatistics {
            state: NetworkState::Connected,
            connected_players,
            current_frame,
            sync_state: SyncState::Synchronized,
            bytes_sent,
            bytes_received,
            uptime: Duration::from_secs(0),
        };
        stats.clone()
    }

    /// Shutdown the network interface
    pub async fn shutdown(&self) {
        log::info!("Shutting down network interface");
        let guard = self.inner.read().await;
        let _ = guard.shutdown().await;
        clear_active_network_interface();
    }

    async fn port(&self) -> u16 {
        let cfg = self.config.read().await;
        cfg.port
    }
}

/// Initialize network - returns wrapped NetworkInterface
pub fn init_network() -> NetworkResult<Arc<RwLock<NetworkInterface>>> {
    // Return a future that will be resolved by the caller
    // This is a synchronous function that returns an async-ready interface
    let interface = NetworkInterface::new(NetworkConfig::default())?;
    let interface = Arc::new(RwLock::new(interface));
    set_active_network_interface(interface.clone());
    Ok(interface)
}

/// Helper function to create network interface with proper async initialization
pub fn create_network_interface(
    config: NetworkConfig,
) -> NetworkResult<Arc<RwLock<NetworkInterface>>> {
    let interface = NetworkInterface::new(config)?;
    let interface = Arc::new(RwLock::new(interface));
    set_active_network_interface(interface.clone());
    Ok(interface)
}

fn to_game_command_data(command: &UnitCommand) -> GameCommandData {
    let mut params: HashMap<String, CommandParameter> = HashMap::new();
    params.insert(
        "unit_count".to_string(),
        CommandParameter::Int(command.unit_ids.len() as i32),
    );
    for (idx, unit_id) in command.unit_ids.iter().enumerate() {
        params.insert(
            format!("unit_{}", idx),
            CommandParameter::ObjectId(*unit_id),
        );
    }

    let command_type = match command.command_type {
        UnitCommandType::Move => CommandType::DoMoveTo as u32,
        UnitCommandType::Attack => CommandType::DoAttackObject as u32,
        UnitCommandType::Stop => CommandType::DoStop as u32,
        UnitCommandType::Guard => CommandType::DoGuardPosition as u32,
        UnitCommandType::Build => CommandType::DozerConstruct as u32,
    };

    GameCommandData {
        command_type,
        target_id: match &command.target {
            Some(CommandTarget::Unit(id)) => Some(*id),
            _ => None,
        },
        position: match &command.target {
            Some(CommandTarget::Position(pos)) => Some((pos.x, 0.0, pos.y)),
            _ => None,
        },
        parameters: params,
        checksum: 0,
    }
}
