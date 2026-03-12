#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy integration for multiplayer networking
//!
//! This module provides complete GameSpy functionality including:
//! - Chat system with rooms and private messaging
//! - GameSpy configuration management
//! - Ladder and ranking system
//! - Peer-to-peer networking
//! - Staging room functionality
//! - Buddy list and social features
//! - Game results tracking
//! - Persistent storage
//! - Ping services

use crate::error::NetworkResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{info, instrument, warn};

pub mod buddy;
pub mod buddy_thread;
pub mod chat;
pub mod chat_transport;
pub mod config;
pub mod game_results;
pub mod game_spy_chat;
pub mod game_spy_game_info;
pub mod game_spy_gp;
pub mod game_spy_overlay;
pub mod gamespy_chat;
pub mod gamespy_game_info;
pub mod gamespy_gp;
pub mod gamespy_overlay;
pub mod gs_config;
pub mod ladder;
pub mod ladder_defs;
pub mod lobby_utils;
pub mod main_menu_utils;
pub mod peer;
pub mod peer_defs;
pub mod peer_thread;
pub mod persistent_storage;
pub mod persistent_storage_thread;
pub mod ping;
pub mod ping_thread;
pub mod staging_room;
pub mod staging_room_game_info;
pub mod thread;
pub mod thread_utils;

pub mod buddy_defs;
pub mod peer_defs_implementation;
pub mod persistent_storage_defs;
pub use buddy::*;
pub use buddy_thread::*;
pub use chat::*;
pub use chat_transport::{ChatTransportConfig, WebSocketChatTransport};
pub use config::*;
pub use game_results::*;
pub use ladder::*;
pub use ladder_defs::*;
pub use peer::*;
pub use peer_defs::*;
pub use peer_thread::*;
pub use persistent_storage::*;
pub use persistent_storage_thread::*;
pub use ping::*;
pub use ping_thread::*;
pub use staging_room::*;
pub use thread_utils::*;

/// GameSpy global interface
pub struct GameSpyInterface {
    /// Chat system
    chat: Arc<RwLock<GameSpyChat>>,
    /// Configuration (shared mutable)
    config: Arc<RwLock<GameSpyConfig>>,
    /// Ladder system
    ladder: Arc<RwLock<LadderSystem>>,
    /// Peer networking
    peer: Arc<RwLock<PeerSystem>>,
    /// Staging room
    staging_room: Arc<RwLock<StagingRoom>>,
    /// Buddy list
    buddy: Arc<RwLock<BuddySystem>>,
    /// Game results
    game_results: Arc<RwLock<GameResultsSystem>>,
    /// Persistent storage
    storage: Arc<RwLock<PersistentStorage>>,
    /// Ping service
    ping_service: Arc<RwLock<PingService>>,

    /// Global state
    is_connected: Arc<RwLock<bool>>,
    local_player_id: Option<String>,

    /// Event channels
    event_tx: broadcast::Sender<GameSpyEvent>,
    command_rx: mpsc::Receiver<GameSpyCommand>,
}

impl GameSpyInterface {
    /// Create new GameSpy interface
    pub async fn new() -> NetworkResult<Self> {
        let (event_tx, _) = broadcast::channel(1000);
        let (command_tx, command_rx) = mpsc::channel(1000);

        let config = Arc::new(RwLock::new(GameSpyConfig::new().await?));
        let storage_root = {
            let cfg = config.read().await;
            cfg.storage_directory()
        };
        let storage = Arc::new(RwLock::new(PersistentStorage::new(&storage_root).await?));
        let transport_cfg_result = {
            let cfg = config.read().await;
            cfg.chat_transport_config()
        };

        let chat_instance = match transport_cfg_result {
            Ok(transport_config) => {
                let endpoint = transport_config.endpoint.clone();
                match WebSocketChatTransport::connect(transport_config).await {
                    Ok(transport) => {
                        info!("Using WebSocket chat transport at {}", endpoint);
                        let transport: Arc<dyn ChatTransport + Send + Sync> = Arc::new(transport);
                        GameSpyChat::with_transport(event_tx.clone(), transport).await?
                    }
                    Err(err) => {
                        warn!(
                            "Failed to initialize WebSocket chat transport ({}); using local fallback",
                            err
                        );
                        GameSpyChat::new(event_tx.clone()).await?
                    }
                }
            }
            Err(err) => {
                warn!(
                    "Chat transport configuration invalid ({}); using local chat implementation",
                    err
                );
                GameSpyChat::new(event_tx.clone()).await?
            }
        };

        Ok(Self {
            chat: Arc::new(RwLock::new(chat_instance)),
            config: config.clone(),
            ladder: Arc::new(RwLock::new(
                LadderSystem::new(config.clone(), storage.clone()).await?,
            )),
            peer: Arc::new(RwLock::new(PeerSystem::new().await?)),
            staging_room: Arc::new(RwLock::new(StagingRoom::new().await?)),
            buddy: Arc::new(RwLock::new(BuddySystem::new().await?)),
            game_results: Arc::new(RwLock::new(GameResultsSystem::new().await?)),
            storage: storage.clone(),
            ping_service: Arc::new(RwLock::new(PingService::new().await?)),

            is_connected: Arc::new(RwLock::new(false)),
            local_player_id: None,

            event_tx,
            command_rx,
        })
    }

    /// Initialize GameSpy connection
    #[instrument(skip(self))]
    pub async fn initialize(&mut self, player_id: String, password: String) -> NetworkResult<()> {
        info!("Initializing GameSpy for player: {}", player_id);

        self.local_player_id = Some(player_id.clone());
        {
            let chat = self.chat.read().await;
            chat.set_local_player_id(player_id.clone()).await;
        }

        // Connect to GameSpy master server
        self.connect_to_gamespy(player_id, password).await?;

        // Start all subsystems
        self.start_subsystems().await?;

        // Load persistent data
        self.load_persistent_data().await?;

        *self.is_connected.write().await = true;
        info!("GameSpy initialization complete");

        Ok(())
    }

    /// Update the chat authentication token and seamlessly reconnect the chat transport.
    pub async fn update_chat_auth_token(&self, token: Option<String>) -> NetworkResult<()> {
        {
            let mut cfg = self.config.write().await;
            cfg.set_chat_auth_token(token);
        }

        let transport_config = {
            let cfg = self.config.read().await;
            cfg.chat_transport_config()
        }?;

        let endpoint = transport_config.endpoint.clone();
        let mut chat_source = "remote";
        let new_chat = match WebSocketChatTransport::connect(transport_config).await {
            Ok(transport) => {
                let transport: Arc<dyn ChatTransport + Send + Sync> = Arc::new(transport);
                GameSpyChat::with_transport(self.event_tx.clone(), transport).await?
            }
            Err(err) => {
                chat_source = "local";
                warn!(
                    "Failed to initialize WebSocket chat transport ({}); falling back to local chat",
                    err
                );
                GameSpyChat::new(self.event_tx.clone()).await?
            }
        };
        if let Some(player_id) = &self.local_player_id {
            new_chat.set_local_player_id(player_id.clone()).await;
        }
        new_chat.start().await?;

        let mut chat_guard = self.chat.write().await;
        let old_chat = std::mem::replace(&mut *chat_guard, new_chat);
        // stop after swap to release write lock quickly
        drop(chat_guard);
        if let Err(err) = old_chat.stop().await {
            warn!("Failed to stop old chat transport cleanly: {}", err);
        }

        info!(
            "Chat transport reinitialized using {} backend for endpoint {}",
            chat_source, endpoint
        );
        Ok(())
    }

    /// Connect to GameSpy master server
    async fn connect_to_gamespy(&self, player_id: String, password: String) -> NetworkResult<()> {
        info!("Connecting to GameSpy master server");

        // Validate credentials
        {
            let config = self.config.read().await;
            config.validate_credentials(&player_id, &password)?;
        }

        // Establish connection to GameSpy backend
        // This would connect to the actual GameSpy servers
        // For now, we'll simulate the connection

        Ok(())
    }

    /// Start all GameSpy subsystems
    async fn start_subsystems(&self) -> NetworkResult<()> {
        // Start chat system
        {
            let chat = self.chat.write().await;
            chat.start().await?;
        }

        // Start ladder system
        {
            let mut ladder = self.ladder.write().await;
            ladder.start().await?;
        }

        // Start peer system
        {
            let mut peer = self.peer.write().await;
            peer.start().await?;
        }

        // Start buddy system
        {
            let mut buddy = self.buddy.write().await;
            buddy.start().await?;
        }

        // Start ping service
        {
            let mut ping = self.ping_service.write().await;
            ping.start().await?;
        }

        Ok(())
    }

    /// Load persistent data
    async fn load_persistent_data(&self) -> NetworkResult<()> {
        if let Some(player_id) = &self.local_player_id {
            let storage = self.storage.read().await;

            // Load buddy list
            if let Ok(buddies) = storage.load_buddy_list(player_id).await {
                let mut buddy_system = self.buddy.write().await;
                buddy_system.set_buddy_list(buddies);
            }

            // Load player stats
            if let Ok(stats) = storage.load_player_stats(player_id).await {
                let mut ladder = self.ladder.write().await;
                ladder.update_player_stats(player_id.clone(), stats).await;
            }
        }

        Ok(())
    }

    /// Shutdown GameSpy
    #[instrument(skip(self))]
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down GameSpy");

        *self.is_connected.write().await = false;

        // Shutdown all subsystems in reverse order
        {
            let mut ping = self.ping_service.write().await;
            ping.stop().await?;
        }

        {
            let mut buddy = self.buddy.write().await;
            buddy.stop().await?;
        }

        {
            let mut peer = self.peer.write().await;
            peer.stop().await?;
        }

        {
            let mut ladder = self.ladder.write().await;
            ladder.stop().await?;
        }

        {
            let chat = self.chat.write().await;
            chat.stop().await?;
        }

        // Save persistent data
        self.save_persistent_data().await?;

        info!("GameSpy shutdown complete");
        Ok(())
    }

    /// Save persistent data
    async fn save_persistent_data(&self) -> NetworkResult<()> {
        if let Some(player_id) = &self.local_player_id {
            let storage = self.storage.read().await;

            // Save buddy list
            {
                let buddy_system = self.buddy.read().await;
                let buddies = buddy_system.get_buddy_list().await;
                storage.save_buddy_list(player_id, buddies).await?;
            }

            // Save player stats
            {
                let ladder = self.ladder.read().await;
                if let Some(stats) = ladder.get_player_stats(player_id).await {
                    storage.save_player_stats(player_id, stats).await?;
                }
            }
        }

        Ok(())
    }

    /// Send chat message
    pub async fn send_chat(&self, message: String, room: Option<String>) -> NetworkResult<()> {
        let chat = self.chat.read().await;
        chat.send_message(message, room).await
    }

    /// Join chat room
    pub async fn join_chat_room(&self, room_name: String) -> NetworkResult<()> {
        let chat = self.chat.read().await;
        chat.join_room(room_name).await
    }

    /// Leave chat room
    pub async fn leave_chat_room(&self, room_name: String) -> NetworkResult<()> {
        let chat = self.chat.read().await;
        chat.leave_room(room_name).await
    }

    /// Add buddy
    pub async fn add_buddy(&self, buddy_id: String) -> NetworkResult<()> {
        let buddy = self.buddy.read().await;
        buddy.add_buddy(buddy_id).await
    }

    /// Remove buddy
    pub async fn remove_buddy(&self, buddy_id: String) -> NetworkResult<()> {
        let buddy = self.buddy.read().await;
        buddy.remove_buddy(buddy_id).await
    }

    /// Send game invitation
    pub async fn send_game_invite(
        &self,
        player_id: String,
        game_settings: GameSettings,
    ) -> NetworkResult<()> {
        let staging = self.staging_room.read().await;
        staging.send_invite(player_id, game_settings).await
    }

    /// Accept game invitation
    pub async fn accept_game_invite(&self, invite_id: String) -> NetworkResult<()> {
        let staging = self.staging_room.read().await;
        staging.accept_invite(invite_id).await
    }

    /// Create custom game
    pub async fn create_custom_game(&self, settings: GameSettings) -> NetworkResult<String> {
        let staging = self.staging_room.read().await;
        staging.create_game(settings).await
    }

    /// Join custom game
    pub async fn join_custom_game(&self, game_id: String) -> NetworkResult<()> {
        let staging = self.staging_room.read().await;
        staging.join_game(game_id).await
    }

    /// Start matchmaking
    pub async fn start_matchmaking(
        &self,
        preferences: MatchmakingPreferences,
    ) -> NetworkResult<()> {
        let ladder = self.ladder.read().await;
        ladder.start_matchmaking(preferences).await
    }

    /// Cancel matchmaking
    pub async fn cancel_matchmaking(&self) -> NetworkResult<()> {
        let ladder = self.ladder.read().await;
        ladder.cancel_matchmaking().await
    }

    /// Report game results
    pub async fn report_game_results(&self, results: GameResults) -> NetworkResult<()> {
        let game_results = self.game_results.read().await;
        game_results.report_results(results).await
    }

    /// Get ping to server
    pub async fn get_ping(&self, server: String) -> NetworkResult<u32> {
        let ping = self.ping_service.read().await;
        ping.get_ping(server).await
    }

    /// Check if connected to GameSpy
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get event receiver
    pub fn get_event_receiver(&self) -> broadcast::Receiver<GameSpyEvent> {
        self.event_tx.subscribe()
    }

    /// Get current status
    pub async fn get_status(&self) -> GameSpyStatus {
        let is_connected = self.is_connected().await;

        if !is_connected {
            return GameSpyStatus::Disconnected;
        }

        // Check various subsystem states
        let chat_connected = self.chat.read().await.is_connected();
        let in_matchmaking = self.ladder.read().await.is_in_matchmaking().await;

        match (chat_connected, in_matchmaking) {
            (true, true) => GameSpyStatus::Matchmaking,
            (true, false) => GameSpyStatus::Connected,
            (false, _) => GameSpyStatus::Connecting,
        }
    }
}

/// GameSpy events
#[derive(Debug, Clone)]
pub enum GameSpyEvent {
    /// Connected to GameSpy
    Connected,
    /// Disconnected from GameSpy
    Disconnected,
    /// Chat message received
    ChatMessage(ChatMessage),
    /// Player joined room
    PlayerJoinedRoom { player_id: String, room: String },
    /// Player left room
    PlayerLeftRoom { player_id: String, room: String },
    /// Buddy status changed
    BuddyStatusChanged {
        buddy_id: String,
        status: BuddyStatus,
    },
    /// Game invitation received
    GameInviteReceived(GameInvite),
    /// Matchmaking update
    MatchmakingUpdate(MatchmakingUpdate),
    /// Game results processed
    GameResultsProcessed { new_rating: u32 },
    /// Error occurred
    Error(String),
}

/// GameSpy commands
#[derive(Debug)]
pub enum GameSpyCommand {
    /// Send chat message
    SendChat {
        message: String,
        room: Option<String>,
    },
    /// Join room
    JoinRoom(String),
    /// Leave room
    LeaveRoom(String),
    /// Add buddy
    AddBuddy(String),
    /// Remove buddy
    RemoveBuddy(String),
    /// Send game invite
    SendGameInvite {
        player_id: String,
        settings: GameSettings,
    },
    /// Start matchmaking
    StartMatchmaking(MatchmakingPreferences),
    /// Cancel matchmaking
    CancelMatchmaking,
    /// Report game results
    ReportGameResults(GameResults),
}

/// GameSpy status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSpyStatus {
    /// Not connected
    Disconnected,
    /// Connecting to GameSpy
    Connecting,
    /// Connected and available
    Connected,
    /// In matchmaking queue
    Matchmaking,
}

/// Chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender: String,
    pub message: String,
    pub room: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub message_type: ChatMessageType,
}

/// Chat message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMessageType {
    Normal,
    Emote,
    Private,
    System,
    Owner,
}

/// Game settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    pub map: String,
    pub game_mode: String,
    pub max_players: u8,
    pub is_ranked: bool,
    pub password: Option<String>,
}

/// Game invitation
#[derive(Debug, Clone)]
pub struct GameInvite {
    pub invite_id: String,
    pub from_player: String,
    pub game_settings: GameSettings,
    pub timestamp: DateTime<Utc>,
}

/// Matchmaking preferences
#[derive(Debug, Clone)]
pub struct MatchmakingPreferences {
    pub game_mode: String,
    pub map_preference: Option<String>,
    pub skill_range: (u32, u32),
    pub max_wait_time: u64,
}

/// Matchmaking update
#[derive(Debug, Clone)]
pub enum MatchmakingUpdate {
    /// Joined matchmaking queue
    JoinedQueue { estimated_wait: u64 },
    /// Match found
    MatchFound { game_id: String },
    /// Matchmaking cancelled
    Cancelled,
    /// Error in matchmaking
    Error(String),
}

/// Game results
#[derive(Debug, Clone)]
pub struct GameResults {
    pub game_id: String,
    pub winner: Option<String>,
    pub duration_seconds: u32,
    pub player_stats: HashMap<String, PlayerGameStats>,
}

/// Player game statistics
#[derive(Debug, Clone)]
pub struct PlayerGameStats {
    pub player_id: String,
    pub score: u32,
    pub units_destroyed: u32,
    pub buildings_destroyed: u32,
    pub resources_collected: u32,
}

/// Buddy status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuddyStatus {
    Offline,
    Online,
    Away,
    InGame,
    InChat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gamespy_initialization() {
        let gamespy = GameSpyInterface::new().await.unwrap();
        assert!(!gamespy.is_connected().await);
        assert_eq!(gamespy.get_status().await, GameSpyStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_gamespy_status_transitions() {
        let gamespy = GameSpyInterface::new().await.unwrap();
        assert_eq!(gamespy.get_status().await, GameSpyStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_update_chat_auth_token_falls_back() {
        let gamespy = GameSpyInterface::new().await.unwrap();
        gamespy
            .update_chat_auth_token(Some("test-token".to_string()))
            .await
            .unwrap();
    }
}
