//! Lobby management for multiplayer games
//!
//! This module provides comprehensive lobby functionality for matchmaking,
//! including player management, game settings, and async coordination.

use crate::error::{NetworkError, NetworkResult};
use crate::security::auth::AuthToken;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Lobby state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyState {
    /// Lobby is open and accepting players
    Open,
    /// Lobby is full but still configuring
    Full,
    /// Game is starting/loading
    Starting,
    /// Game is in progress
    InGame,
    /// Lobby is closed
    Closed,
}

/// Player information in lobby
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyPlayer {
    /// Player unique ID
    pub id: Uuid,
    /// Player display name
    pub name: String,
    /// Player position in lobby (0-7)
    pub position: u8,
    /// Player ready state
    pub ready: bool,
    /// Authentication token
    pub auth_token: AuthToken,
    /// Player rank/rating
    pub rank: u32,
    /// Connection timestamp
    pub joined_at: SystemTime,
}

/// Game configuration for the lobby
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    /// Map name
    pub map_name: String,
    /// Maximum number of players
    pub max_players: u8,
    /// Game speed multiplier
    pub game_speed: f32,
    /// Starting resources
    pub starting_resources: u32,
    /// Game mode specific settings
    pub game_mode_settings: HashMap<String, String>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            map_name: "default_map".to_string(),
            max_players: 8,
            game_speed: 1.0,
            starting_resources: 10000,
            game_mode_settings: HashMap::new(),
        }
    }
}

/// Lobby event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyEvent {
    /// Player joined the lobby
    PlayerJoined(LobbyPlayer),
    /// Player left the lobby
    PlayerLeft(Uuid),
    /// Player changed ready state
    PlayerReadyChanged(Uuid, bool),
    /// Game configuration changed
    ConfigChanged(GameConfig),
    /// Lobby state changed
    StateChanged(LobbyState),
    /// Game is starting
    GameStarting,
    /// Chat message
    ChatMessage { player_id: Uuid, message: String },
}

/// Game lobby for multiplayer sessions
pub struct Lobby {
    /// Unique lobby identifier
    id: Uuid,
    /// Lobby name
    name: String,
    /// Current lobby state
    state: Arc<RwLock<LobbyState>>,
    /// Players in the lobby
    players: Arc<RwLock<HashMap<Uuid, LobbyPlayer>>>,
    /// Game configuration
    config: Arc<RwLock<GameConfig>>,
    /// Host player ID
    host_id: Arc<RwLock<Option<Uuid>>>,

    // Event broadcasting
    event_tx: broadcast::Sender<LobbyEvent>,

    // Background tasks
    heartbeat_task: Option<JoinHandle<()>>,
    timeout_task: Option<JoinHandle<()>>,
    shutdown_tx: broadcast::Sender<()>,

    // Configuration
    lobby_timeout: Duration,
    heartbeat_interval: Duration,

    // Statistics
    created_at: SystemTime,
    last_activity: Arc<RwLock<SystemTime>>,
}

impl Lobby {
    /// Create a new lobby
    pub fn new(name: String, host_name: String, host_auth: AuthToken) -> NetworkResult<Self> {
        let lobby_id = Uuid::new_v4();
        let host_id = Uuid::new_v4();

        // Create host player
        let host_player = LobbyPlayer {
            id: host_id,
            name: host_name,
            position: 0,
            ready: false,
            auth_token: host_auth,
            rank: 1000, // Default rank
            joined_at: SystemTime::now(),
        };

        let mut players = HashMap::new();
        players.insert(host_id, host_player);

        let (event_tx, _) = broadcast::channel(100);
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            id: lobby_id,
            name,
            state: Arc::new(RwLock::new(LobbyState::Open)),
            players: Arc::new(RwLock::new(players)),
            config: Arc::new(RwLock::new(GameConfig::default())),
            host_id: Arc::new(RwLock::new(Some(host_id))),
            event_tx,
            heartbeat_task: None,
            timeout_task: None,
            shutdown_tx,
            lobby_timeout: Duration::from_secs(300), // 5 minutes
            heartbeat_interval: Duration::from_secs(30),
            created_at: SystemTime::now(),
            last_activity: Arc::new(RwLock::new(SystemTime::now())),
        })
    }

    /// Start lobby background tasks
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting lobby '{}' with ID {}", self.name, self.id);

        // Start heartbeat task
        self.start_heartbeat_task().await;

        // Start timeout task
        self.start_timeout_task().await;

        // Broadcast lobby creation
        let _ = self
            .event_tx
            .send(LobbyEvent::StateChanged(LobbyState::Open));

        Ok(())
    }

    /// Start heartbeat task for keep-alive
    async fn start_heartbeat_task(&mut self) {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let interval = self.heartbeat_interval;

        let handle = tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        // Send heartbeat (could be used for lobby discovery)
                        debug!("Lobby heartbeat");
                    }

                    _ = shutdown_rx.recv() => {
                        debug!("Heartbeat task shutting down");
                        break;
                    }
                }
            }
        });

        self.heartbeat_task = Some(handle);
    }

    /// Start timeout task for lobby cleanup
    async fn start_timeout_task(&mut self) {
        let state = self.state.clone();
        let last_activity = self.last_activity.clone();
        let timeout_duration = self.lobby_timeout;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut check_interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        let current_state = *state.read().await;
                        let last_activity_time = *last_activity.read().await;

                        // Check if lobby should timeout (only if open and inactive)
                        if matches!(current_state, LobbyState::Open | LobbyState::Full) {
                            if last_activity_time.elapsed().unwrap_or(Duration::from_secs(0)) > timeout_duration {
                                warn!("Lobby timeout reached, closing");
                                let mut state_lock = state.write().await;
                                *state_lock = LobbyState::Closed;
                            }
                        }
                    }

                    _ = shutdown_rx.recv() => {
                        debug!("Timeout task shutting down");
                        break;
                    }
                }
            }
        });

        self.timeout_task = Some(handle);
    }

    /// Join lobby
    pub async fn join(&self, player_name: String, auth_token: AuthToken) -> NetworkResult<Uuid> {
        let mut players = self.players.write().await;
        let mut state = self.state.write().await;

        // Check if lobby is open
        if !matches!(*state, LobbyState::Open) {
            return Err(NetworkError::generic(
                "Lobby is not accepting new players".to_string(),
            ));
        }

        // Check if lobby is full
        let config = self.config.read().await;
        if players.len() >= config.max_players as usize {
            *state = LobbyState::Full;
            return Err(NetworkError::generic("Lobby is full".to_string()));
        }

        // Find next available position
        let position = self.find_next_position(&players)?;

        // Create new player
        let player_id = Uuid::new_v4();
        let player = LobbyPlayer {
            id: player_id,
            name: player_name.clone(),
            position,
            ready: false,
            auth_token,
            rank: 1000, // Default rank
            joined_at: SystemTime::now(),
        };

        players.insert(player_id, player.clone());

        // Update lobby state if full
        if players.len() >= config.max_players as usize {
            *state = LobbyState::Full;
        }

        // Update activity
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = SystemTime::now();
        }

        // Broadcast player joined event
        let _ = self.event_tx.send(LobbyEvent::PlayerJoined(player));
        if matches!(*state, LobbyState::Full) {
            let _ = self
                .event_tx
                .send(LobbyEvent::StateChanged(LobbyState::Full));
        }

        info!(
            "Player '{}' joined lobby at position {}",
            player_name, position
        );

        Ok(player_id)
    }

    /// Leave lobby
    pub async fn leave(&self, player_id: Uuid) -> NetworkResult<()> {
        let mut players = self.players.write().await;
        let mut state = self.state.write().await;

        let player = players
            .remove(&player_id)
            .ok_or_else(|| NetworkError::generic("Player not found in lobby".to_string()))?;

        // If this was the host, transfer host to another player
        {
            let mut host_id = self.host_id.write().await;
            if *host_id == Some(player_id) {
                *host_id = players.keys().next().copied();
                if let Some(new_host_id) = *host_id {
                    info!("Host transferred to player {}", new_host_id);
                }
            }
        }

        // Update lobby state
        if matches!(*state, LobbyState::Full) && !players.is_empty() {
            *state = LobbyState::Open;
        } else if players.is_empty() {
            *state = LobbyState::Closed;
        }

        // Update activity
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = SystemTime::now();
        }

        // Broadcast events
        let _ = self.event_tx.send(LobbyEvent::PlayerLeft(player_id));
        if matches!(*state, LobbyState::Open) {
            let _ = self
                .event_tx
                .send(LobbyEvent::StateChanged(LobbyState::Open));
        }

        info!("Player '{}' left lobby", player.name);

        Ok(())
    }

    /// Set player ready state
    pub async fn set_player_ready(&self, player_id: Uuid, ready: bool) -> NetworkResult<()> {
        let mut players = self.players.write().await;

        let player_name = {
            let player = players
                .get_mut(&player_id)
                .ok_or_else(|| NetworkError::generic("Player not found in lobby".to_string()))?;

            player.ready = ready;
            player.name.clone()
        };

        // Update activity
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = SystemTime::now();
        }

        // Broadcast ready state change
        let _ = self
            .event_tx
            .send(LobbyEvent::PlayerReadyChanged(player_id, ready));

        // Check if all players are ready
        let all_ready = players.values().all(|p| p.ready);
        if all_ready && players.len() > 1 {
            // Need at least 2 players
            info!("All players ready, game can start");
        }

        info!("Player '{}' ready state: {}", player_name, ready);

        Ok(())
    }

    /// Update game configuration (host only)
    pub async fn update_config(
        &self,
        player_id: Uuid,
        new_config: GameConfig,
    ) -> NetworkResult<()> {
        // Verify player is host
        let host_id = self.host_id.read().await;
        if *host_id != Some(player_id) {
            return Err(NetworkError::generic(
                "Only host can change game configuration".to_string(),
            ));
        }

        // Update configuration
        {
            let mut config = self.config.write().await;
            *config = new_config.clone();
        }

        // Update activity
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = SystemTime::now();
        }

        // Broadcast configuration change
        let _ = self.event_tx.send(LobbyEvent::ConfigChanged(new_config));

        info!("Game configuration updated by host");

        Ok(())
    }

    /// Start game (host only)
    pub async fn start_game(&self, player_id: Uuid) -> NetworkResult<()> {
        // Verify player is host
        let host_id = self.host_id.read().await;
        if *host_id != Some(player_id) {
            return Err(NetworkError::generic(
                "Only host can start the game".to_string(),
            ));
        }

        // Check if all players are ready
        let players = self.players.read().await;
        let all_ready = players.values().all(|p| p.ready);

        if !all_ready {
            return Err(NetworkError::generic(
                "Not all players are ready".to_string(),
            ));
        }

        if players.len() < 2 {
            return Err(NetworkError::generic(
                "Need at least 2 players to start".to_string(),
            ));
        }

        // Change state to starting
        {
            let mut state = self.state.write().await;
            *state = LobbyState::Starting;
        }

        // Broadcast game starting
        let _ = self
            .event_tx
            .send(LobbyEvent::StateChanged(LobbyState::Starting));
        let _ = self.event_tx.send(LobbyEvent::GameStarting);

        info!("Game starting with {} players", players.len());

        // Transition to in-game after a delay (simulating loading)
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;

            {
                let mut state_lock = state.write().await;
                *state_lock = LobbyState::InGame;
            }

            let _ = event_tx.send(LobbyEvent::StateChanged(LobbyState::InGame));
        });

        Ok(())
    }

    /// Send chat message
    pub async fn send_chat(&self, player_id: Uuid, message: String) -> NetworkResult<()> {
        // Verify player is in lobby
        let players = self.players.read().await;
        if !players.contains_key(&player_id) {
            return Err(NetworkError::generic("Player not in lobby".to_string()));
        }

        // Update activity
        {
            let mut last_activity = self.last_activity.write().await;
            *last_activity = SystemTime::now();
        }

        // Broadcast chat message
        let _ = self
            .event_tx
            .send(LobbyEvent::ChatMessage { player_id, message });

        Ok(())
    }

    /// Subscribe to lobby events
    pub fn subscribe_events(&self) -> broadcast::Receiver<LobbyEvent> {
        self.event_tx.subscribe()
    }

    /// Get lobby information
    pub async fn get_info(&self) -> LobbyInfo {
        let state = *self.state.read().await;
        let players: Vec<LobbyPlayer> = self.players.read().await.values().cloned().collect();
        let player_count = players.len() as u8;
        let config = self.config.read().await.clone();
        let host_id = *self.host_id.read().await;

        LobbyInfo {
            id: self.id,
            name: self.name.clone(),
            state,
            players,
            config,
            host_id,
            created_at_secs: self
                .created_at
                .elapsed()
                .unwrap_or(Duration::from_secs(0))
                .as_secs(),
            player_count,
        }
    }

    /// Shutdown lobby
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down lobby '{}'", self.name);

        // Signal shutdown to background tasks
        let _ = self.shutdown_tx.send(());

        // Wait for tasks to complete
        if let Some(handle) = self.heartbeat_task.take() {
            handle.abort();
            let _ = handle.await;
        }

        if let Some(handle) = self.timeout_task.take() {
            handle.abort();
            let _ = handle.await;
        }

        // Close lobby
        {
            let mut state = self.state.write().await;
            *state = LobbyState::Closed;
        }

        // Broadcast closure
        let _ = self
            .event_tx
            .send(LobbyEvent::StateChanged(LobbyState::Closed));

        Ok(())
    }

    /// Find next available position in lobby
    fn find_next_position(&self, players: &HashMap<Uuid, LobbyPlayer>) -> NetworkResult<u8> {
        for position in 0..8 {
            // Max 8 players
            if !players.values().any(|p| p.position == position) {
                return Ok(position);
            }
        }

        Err(NetworkError::generic(
            "No available positions in lobby".to_string(),
        ))
    }

    /// Get lobby ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get lobby name
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Lobby information snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInfo {
    /// Lobby ID
    pub id: Uuid,
    /// Lobby name
    pub name: String,
    /// Current state
    pub state: LobbyState,
    /// Players in lobby
    pub players: Vec<LobbyPlayer>,
    /// Game configuration
    pub config: GameConfig,
    /// Host player ID
    pub host_id: Option<Uuid>,
    /// Creation timestamp (seconds since Unix epoch)
    pub created_at_secs: u64,
    /// Current player count
    pub player_count: u8,
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new(
            "Default Lobby".to_string(),
            "Host".to_string(),
            AuthToken::default(),
        )
        .unwrap()
    }
}
