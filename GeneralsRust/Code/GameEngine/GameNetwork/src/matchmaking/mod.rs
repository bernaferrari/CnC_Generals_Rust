//! Modern matchmaking system to replace GameSpy functionality
//!
//! This module provides cloud-based matchmaking services including
//! game lobbies, player matching, server browser, and social features.

use crate::error::{NetworkError, NetworkResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

pub mod browser;
pub mod lobby;
pub mod matchmaker;
pub mod ranking;
pub mod slots;
pub mod social;

/// Matchmaking service configuration
#[derive(Debug, Clone)]
pub struct MatchmakingConfig {
    /// Service endpoint URL
    pub service_url: String,
    /// API key for authentication
    pub api_key: String,
    /// Enable ranked matchmaking
    pub enable_ranked: bool,
    /// Enable custom lobbies
    pub enable_custom_lobbies: bool,
    /// Maximum lobby size
    pub max_lobby_size: usize,
    /// Matchmaking timeout
    pub matchmaking_timeout_seconds: u64,
    /// Enable Discord integration
    pub enable_discord: bool,
    /// Regional server preference
    pub preferred_region: String,
}

impl Default for MatchmakingConfig {
    fn default() -> Self {
        Self {
            service_url: "https://api.generals-remastered.com".to_string(),
            api_key: String::new(),
            enable_ranked: true,
            enable_custom_lobbies: true,
            max_lobby_size: 8,
            matchmaking_timeout_seconds: 120,
            enable_discord: true,
            preferred_region: "auto".to_string(),
        }
    }
}

/// Player information for matchmaking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchmakingPlayer {
    /// Unique player ID
    pub player_id: Uuid,
    /// Display name
    pub display_name: String,
    /// Player level/rank
    pub rank: PlayerRank,
    /// Skill rating (for ranked games)
    pub skill_rating: u32,
    /// Preferred faction
    pub preferred_faction: Option<String>,
    /// Player status
    pub status: PlayerStatus,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Player statistics
    pub stats: PlayerStats,
}

/// Player rank/level information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRank {
    /// Rank tier (Bronze, Silver, Gold, etc.)
    pub tier: RankTier,
    /// Division within tier (1-5)
    pub division: u8,
    /// League points
    pub points: u32,
}

/// Rank tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum RankTier {
    Unranked = 0,
    Bronze = 1,
    Silver = 2,
    Gold = 3,
    Platinum = 4,
    Diamond = 5,
    Master = 6,
    Grandmaster = 7,
}

impl std::fmt::Display for RankTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Unranked => "Unranked",
            Self::Bronze => "Bronze",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
            Self::Platinum => "Platinum",
            Self::Diamond => "Diamond",
            Self::Master => "Master",
            Self::Grandmaster => "Grandmaster",
        };
        write!(f, "{}", name)
    }
}

/// Player status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlayerStatus {
    /// Offline
    Offline = 0,
    /// Online and available
    Online = 1,
    /// Away/idle
    Away = 2,
    /// In matchmaking queue
    Matchmaking = 3,
    /// In game lobby
    InLobby = 4,
    /// Playing a game
    InGame = 5,
    /// Spectating a game
    Spectating = 6,
}

/// Player statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    /// Total games played
    pub games_played: u32,
    /// Games won
    pub games_won: u32,
    /// Games lost
    pub games_lost: u32,
    /// Win rate percentage
    pub win_rate: f64,
    /// Average game duration (minutes)
    pub avg_game_duration: f64,
    /// Favorite faction
    pub favorite_faction: Option<String>,
    /// Total playtime (hours)
    pub total_playtime_hours: f64,
}

impl PlayerStats {
    /// Calculate win rate
    pub fn calculate_win_rate(&mut self) {
        if self.games_played > 0 {
            self.win_rate = (self.games_won as f64 / self.games_played as f64) * 100.0;
        }
    }
}

/// Game lobby information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLobby {
    /// Unique lobby identifier
    pub lobby_id: Uuid,
    /// Lobby name
    pub name: String,
    /// Host player
    pub host_player: Uuid,
    /// Current players in lobby
    pub players: Vec<MatchmakingPlayer>,
    /// Maximum players allowed
    pub max_players: usize,
    /// Game mode
    pub game_mode: GameMode,
    /// Map name
    pub map: String,
    /// Lobby settings
    pub settings: LobbySettings,
    /// Lobby status
    pub status: LobbyStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Password protection
    pub is_password_protected: bool,
    /// Spectator slots
    pub spectator_slots: u8,
    /// Current spectators
    pub spectators: Vec<Uuid>,
    /// Server region
    pub region: String,
}

/// Game modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum GameMode {
    /// Skirmish vs AI
    Skirmish = 0,
    /// Multiplayer PvP
    Multiplayer = 1,
    /// Tournament match
    Tournament = 2,
    /// Custom scenario
    Custom = 3,
    /// Training/tutorial
    Training = 4,
}

/// Lobby settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySettings {
    /// Starting resources multiplier
    pub starting_resources: f32,
    /// Game speed multiplier
    pub game_speed: f32,
    /// Enable superweapons
    pub allow_superweapons: bool,
    /// Enable cash bounty
    pub enable_cash_bounty: bool,
    /// Unit limit per player
    pub unit_limit: u32,
    /// Tech level restriction
    pub tech_level: u8,
    /// Ranked game
    pub is_ranked: bool,
}

impl Default for LobbySettings {
    fn default() -> Self {
        Self {
            starting_resources: 1.0,
            game_speed: 1.0,
            allow_superweapons: true,
            enable_cash_bounty: false,
            unit_limit: 100,
            tech_level: 5, // Max tech level
            is_ranked: false,
        }
    }
}

/// Lobby status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LobbyStatus {
    /// Waiting for players
    WaitingForPlayers = 0,
    /// All players ready, starting soon
    Starting = 1,
    /// Game in progress
    InProgress = 2,
    /// Game finished
    Finished = 3,
    /// Lobby closed
    Closed = 4,
}

/// Matchmaking queue entry
#[derive(Debug, Clone)]
pub struct MatchmakingQueue {
    /// Player in queue
    pub player: MatchmakingPlayer,
    /// Requested game mode
    pub game_mode: GameMode,
    /// Map preferences
    pub map_preferences: Vec<String>,
    /// Queue entry timestamp
    pub queued_at: DateTime<Utc>,
    /// Acceptable skill rating range
    pub skill_range: (u32, u32),
    /// Regional preference
    pub region_preference: String,
}

/// Matchmaking service events
#[derive(Debug, Clone)]
pub enum MatchmakingEvent {
    /// Player joined matchmaking
    PlayerQueued {
        player_id: Uuid,
        game_mode: GameMode,
    },
    /// Match found for player
    MatchFound { player_id: Uuid, lobby_id: Uuid },
    /// Player left queue
    PlayerLeft { player_id: Uuid, reason: String },
    /// Lobby created
    LobbyCreated { lobby_id: Uuid, host_id: Uuid },
    /// Player joined lobby
    PlayerJoinedLobby { player_id: Uuid, lobby_id: Uuid },
    /// Player left lobby
    PlayerLeftLobby {
        player_id: Uuid,
        lobby_id: Uuid,
        reason: String,
    },
    /// Game started
    GameStarted { lobby_id: Uuid, players: Vec<Uuid> },
    /// Game finished
    GameFinished {
        lobby_id: Uuid,
        winner: Option<Uuid>,
        duration_minutes: u32,
    },
}

/// Matchmaking service implementation
pub struct MatchmakingService {
    /// Configuration
    config: MatchmakingConfig,

    /// Active lobbies
    lobbies: Arc<RwLock<HashMap<Uuid, GameLobby>>>,

    /// Matchmaking queues
    queues: Arc<RwLock<HashMap<GameMode, Vec<MatchmakingQueue>>>>,

    /// Online players
    online_players: Arc<RwLock<HashMap<Uuid, MatchmakingPlayer>>>,

    /// Event callbacks
    event_callback: Option<Arc<dyn MatchmakingEventCallback + Send + Sync>>,

    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Trait for matchmaking event callbacks
pub trait MatchmakingEventCallback {
    /// Called when a matchmaking event occurs
    fn on_event(&self, event: MatchmakingEvent);
}

impl MatchmakingService {
    /// Create new matchmaking service
    pub fn new() -> Self {
        Self::with_config(MatchmakingConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: MatchmakingConfig) -> Self {
        Self {
            config,
            lobbies: Arc::new(RwLock::new(HashMap::new())),
            queues: Arc::new(RwLock::new(HashMap::new())),
            online_players: Arc::new(RwLock::new(HashMap::new())),
            event_callback: None,
            task_handles: Vec::new(),
        }
    }

    /// Set event callback
    pub fn set_event_callback(
        &mut self,
        callback: Arc<dyn MatchmakingEventCallback + Send + Sync>,
    ) {
        self.event_callback = Some(callback);
    }

    /// Start matchmaking service
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting matchmaking service");

        // Start background tasks
        self.start_background_tasks().await?;

        Ok(())
    }

    /// Start background processing tasks
    async fn start_background_tasks(&mut self) -> NetworkResult<()> {
        // Matchmaking processor task
        let queues = self.queues.clone();
        let lobbies = self.lobbies.clone();
        let config = self.config.clone();
        let event_callback = self.event_callback.clone();

        let matchmaking_task = tokio::spawn(async move {
            Self::matchmaking_processor_task(queues, lobbies, config, event_callback).await;
        });

        self.task_handles.push(matchmaking_task);

        // Lobby cleanup task
        let lobbies_cleanup = self.lobbies.clone();
        let cleanup_task = tokio::spawn(async move {
            Self::lobby_cleanup_task(lobbies_cleanup).await;
        });

        self.task_handles.push(cleanup_task);

        Ok(())
    }

    /// Register player online
    pub async fn register_player(&self, player: MatchmakingPlayer) -> NetworkResult<()> {
        let mut online_players = self.online_players.write().await;
        online_players.insert(player.player_id, player.clone());

        info!("Player {} registered online", player.display_name);
        Ok(())
    }

    /// Unregister player (going offline)
    pub async fn unregister_player(&self, player_id: Uuid) -> NetworkResult<()> {
        // Remove from online players
        {
            let mut online_players = self.online_players.write().await;
            online_players.remove(&player_id);
        }

        // Remove from any queues
        {
            let mut queues = self.queues.write().await;
            for queue_list in queues.values_mut() {
                queue_list.retain(|entry| entry.player.player_id != player_id);
            }
        }

        // Remove from lobbies
        {
            let mut lobbies = self.lobbies.write().await;
            let mut lobbies_to_remove = Vec::new();

            for (lobby_id, lobby) in lobbies.iter_mut() {
                // Remove player from lobby
                lobby.players.retain(|p| p.player_id != player_id);
                lobby.spectators.retain(|&id| id != player_id);

                // If host left, transfer host or close lobby
                if lobby.host_player == player_id {
                    if let Some(new_host) = lobby.players.first() {
                        lobby.host_player = new_host.player_id;
                        info!(
                            "Transferred lobby {} host to {}",
                            lobby_id, new_host.display_name
                        );
                    } else {
                        // No players left, mark for removal
                        lobbies_to_remove.push(*lobby_id);
                    }
                }
            }

            // Remove empty lobbies
            for lobby_id in lobbies_to_remove {
                lobbies.remove(&lobby_id);
            }
        }

        info!("Player {} unregistered", player_id);
        Ok(())
    }

    /// Queue player for matchmaking
    pub async fn queue_for_matchmaking(
        &self,
        player_id: Uuid,
        game_mode: GameMode,
        map_preferences: Vec<String>,
    ) -> NetworkResult<()> {
        // Get player info
        let player = {
            let online_players = self.online_players.read().await;
            online_players
                .get(&player_id)
                .cloned()
                .ok_or_else(|| NetworkError::matchmaking("player not online"))?
        };

        // Create queue entry
        let queue_entry = MatchmakingQueue {
            player: player.clone(),
            game_mode,
            map_preferences,
            queued_at: Utc::now(),
            skill_range: (
                player.skill_rating.saturating_sub(200),
                player.skill_rating + 200,
            ),
            region_preference: self.config.preferred_region.clone(),
        };

        // Add to queue
        {
            let mut queues = self.queues.write().await;
            let queue_list = queues.entry(game_mode).or_insert_with(Vec::new);
            queue_list.push(queue_entry);
        }

        // Update player status
        {
            let mut online_players = self.online_players.write().await;
            if let Some(player_entry) = online_players.get_mut(&player_id) {
                player_entry.status = PlayerStatus::Matchmaking;
            }
        }

        // Fire event
        if let Some(callback) = &self.event_callback {
            callback.on_event(MatchmakingEvent::PlayerQueued {
                player_id,
                game_mode,
            });
        }

        info!(
            "Player {} queued for {:?} matchmaking",
            player.display_name, game_mode
        );
        Ok(())
    }

    /// Remove player from matchmaking queue
    pub async fn leave_queue(&self, player_id: Uuid, reason: String) -> NetworkResult<()> {
        // Remove from all queues
        {
            let mut queues = self.queues.write().await;
            for queue_list in queues.values_mut() {
                queue_list.retain(|entry| entry.player.player_id != player_id);
            }
        }

        // Update player status
        {
            let mut online_players = self.online_players.write().await;
            if let Some(player) = online_players.get_mut(&player_id) {
                player.status = PlayerStatus::Online;
            }
        }

        // Fire event
        if let Some(callback) = &self.event_callback {
            callback.on_event(MatchmakingEvent::PlayerLeft {
                player_id,
                reason: reason.clone(),
            });
        }

        info!("Player {} left queue: {}", player_id, reason);
        Ok(())
    }

    /// Create custom lobby
    pub async fn create_lobby(
        &self,
        host_id: Uuid,
        name: String,
        game_mode: GameMode,
        map: String,
        settings: LobbySettings,
        max_players: usize,
        password: Option<String>,
    ) -> NetworkResult<Uuid> {
        if !self.config.enable_custom_lobbies {
            return Err(NetworkError::matchmaking("custom lobbies disabled"));
        }

        // Get host player info
        let host_player = {
            let online_players = self.online_players.read().await;
            online_players
                .get(&host_id)
                .cloned()
                .ok_or_else(|| NetworkError::matchmaking("host player not online"))?
        };

        // Create lobby
        let lobby_id = Uuid::new_v4();
        let lobby = GameLobby {
            lobby_id,
            name: name.clone(),
            host_player: host_id,
            players: vec![host_player],
            max_players: max_players.min(self.config.max_lobby_size),
            game_mode,
            map,
            settings,
            status: LobbyStatus::WaitingForPlayers,
            created_at: Utc::now(),
            is_password_protected: password.is_some(),
            spectator_slots: 4, // Default spectator slots
            spectators: Vec::new(),
            region: self.config.preferred_region.clone(),
        };

        // Store lobby
        {
            let mut lobbies = self.lobbies.write().await;
            lobbies.insert(lobby_id, lobby);
        }

        // Update host status
        {
            let mut online_players = self.online_players.write().await;
            if let Some(player) = online_players.get_mut(&host_id) {
                player.status = PlayerStatus::InLobby;
            }
        }

        // Fire event
        if let Some(callback) = &self.event_callback {
            callback.on_event(MatchmakingEvent::LobbyCreated { lobby_id, host_id });
        }

        info!("Created lobby '{}' hosted by {}", name, host_id);
        Ok(lobby_id)
    }

    /// Join lobby
    pub async fn join_lobby(
        &self,
        player_id: Uuid,
        lobby_id: Uuid,
        password: Option<String>,
    ) -> NetworkResult<()> {
        // Get player info
        let player = {
            let online_players = self.online_players.read().await;
            online_players
                .get(&player_id)
                .cloned()
                .ok_or_else(|| NetworkError::matchmaking("player not online"))?
        };

        // Add to lobby
        {
            let mut lobbies = self.lobbies.write().await;
            let lobby = lobbies
                .get_mut(&lobby_id)
                .ok_or_else(|| NetworkError::matchmaking("lobby not found"))?;

            // Check lobby status
            if lobby.status != LobbyStatus::WaitingForPlayers {
                return Err(NetworkError::matchmaking("lobby not accepting players"));
            }

            // Check if lobby is full
            if lobby.players.len() >= lobby.max_players {
                return Err(NetworkError::matchmaking("lobby is full"));
            }

            // Check password
            if lobby.is_password_protected && password.is_none() {
                return Err(NetworkError::matchmaking("password required"));
            }

            // Check if player already in lobby
            if lobby.players.iter().any(|p| p.player_id == player_id) {
                return Err(NetworkError::matchmaking("player already in lobby"));
            }

            // Add player
            lobby.players.push(player);
        }

        // Update player status
        {
            let mut online_players = self.online_players.write().await;
            if let Some(player_entry) = online_players.get_mut(&player_id) {
                player_entry.status = PlayerStatus::InLobby;
            }
        }

        // Fire event
        if let Some(callback) = &self.event_callback {
            callback.on_event(MatchmakingEvent::PlayerJoinedLobby {
                player_id,
                lobby_id,
            });
        }

        info!("Player {} joined lobby {}", player_id, lobby_id);
        Ok(())
    }

    /// Leave lobby
    pub async fn leave_lobby(
        &self,
        player_id: Uuid,
        lobby_id: Uuid,
        reason: String,
    ) -> NetworkResult<()> {
        {
            let mut lobbies = self.lobbies.write().await;
            if let Some(lobby) = lobbies.get_mut(&lobby_id) {
                // Remove player
                lobby.players.retain(|p| p.player_id != player_id);
                lobby.spectators.retain(|&id| id != player_id);

                // Handle host leaving
                if lobby.host_player == player_id {
                    if let Some(new_host) = lobby.players.first() {
                        lobby.host_player = new_host.player_id;
                        info!(
                            "Transferred lobby {} host to {}",
                            lobby_id, new_host.display_name
                        );
                    } else {
                        // No players left, mark lobby as closed
                        lobby.status = LobbyStatus::Closed;
                    }
                }
            }
        }

        // Update player status
        {
            let mut online_players = self.online_players.write().await;
            if let Some(player) = online_players.get_mut(&player_id) {
                player.status = PlayerStatus::Online;
            }
        }

        // Fire event
        if let Some(callback) = &self.event_callback {
            callback.on_event(MatchmakingEvent::PlayerLeftLobby {
                player_id,
                lobby_id,
                reason: reason.clone(),
            });
        }

        info!("Player {} left lobby {}: {}", player_id, lobby_id, reason);
        Ok(())
    }

    /// Get lobby list
    pub async fn get_lobbies(&self, filter: LobbyFilter) -> Vec<GameLobby> {
        let lobbies = self.lobbies.read().await;

        lobbies
            .values()
            .filter(|lobby| filter.matches(lobby))
            .cloned()
            .collect()
    }

    /// Get online players count
    pub async fn get_online_players_count(&self) -> usize {
        let online_players = self.online_players.read().await;
        online_players.len()
    }

    /// Matchmaking processor background task
    async fn matchmaking_processor_task(
        queues: Arc<RwLock<HashMap<GameMode, Vec<MatchmakingQueue>>>>,
        lobbies: Arc<RwLock<HashMap<Uuid, GameLobby>>>,
        config: MatchmakingConfig,
        event_callback: Option<Arc<dyn MatchmakingEventCallback + Send + Sync>>,
    ) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Process matchmaking for each game mode
            let mut queues_lock = queues.write().await;
            let mut lobbies_lock = lobbies.write().await;

            for (game_mode, queue_list) in queues_lock.iter_mut() {
                if queue_list.len() >= 2 {
                    // Try to match players
                    if let Some(matched_players) = Self::find_match(queue_list) {
                        // Create lobby for matched players
                        let lobby_id = Uuid::new_v4();
                        let host_player = matched_players[0].clone();

                        let lobby = GameLobby {
                            lobby_id,
                            name: format!("Ranked Match - {:?}", game_mode),
                            host_player: host_player.player_id,
                            players: matched_players.clone(),
                            max_players: matched_players.len(),
                            game_mode: *game_mode,
                            map: "Random".to_string(), // Would select appropriate map
                            settings: LobbySettings {
                                is_ranked: config.enable_ranked,
                                ..Default::default()
                            },
                            status: LobbyStatus::Starting,
                            created_at: Utc::now(),
                            is_password_protected: false,
                            spectator_slots: 0,
                            spectators: Vec::new(),
                            region: config.preferred_region.clone(),
                        };

                        lobbies_lock.insert(lobby_id, lobby);

                        // Remove matched players from queue
                        for player in &matched_players {
                            queue_list.retain(|entry| entry.player.player_id != player.player_id);
                        }

                        // Fire events
                        if let Some(callback) = &event_callback {
                            for player in &matched_players {
                                callback.on_event(MatchmakingEvent::MatchFound {
                                    player_id: player.player_id,
                                    lobby_id,
                                });
                            }
                        }

                        info!(
                            "Created ranked match {} for {} players",
                            lobby_id,
                            matched_players.len()
                        );
                    }
                }
            }
        }
    }

    /// Find suitable match from queue
    fn find_match(queue_list: &[MatchmakingQueue]) -> Option<Vec<MatchmakingPlayer>> {
        if queue_list.len() < 2 {
            return None;
        }

        // Simple matching: find players with similar skill ratings
        let first_player = &queue_list[0];
        let mut matched_players = vec![first_player.player.clone()];

        for entry in queue_list.iter().skip(1) {
            // Check skill rating compatibility
            let skill_diff =
                (first_player.player.skill_rating as i32 - entry.player.skill_rating as i32).abs();

            if skill_diff <= 300 {
                // Within 300 skill rating points
                matched_players.push(entry.player.clone());

                if matched_players.len() >= 2 {
                    break; // Found enough players for a match
                }
            }
        }

        if matched_players.len() >= 2 {
            Some(matched_players)
        } else {
            None
        }
    }

    /// Lobby cleanup background task
    async fn lobby_cleanup_task(lobbies: Arc<RwLock<HashMap<Uuid, GameLobby>>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            let mut lobbies_lock = lobbies.write().await;
            let now = Utc::now();

            // Remove old/empty lobbies
            lobbies_lock.retain(|_, lobby| {
                let age_minutes = now.signed_duration_since(lobby.created_at).num_minutes();

                // Keep lobbies that are:
                // - Not empty
                // - Less than 30 minutes old
                // - Not closed
                !lobby.players.is_empty() && age_minutes < 30 && lobby.status != LobbyStatus::Closed
            });
        }
    }
}

/// Lobby filter for searching
#[derive(Debug, Clone, Default)]
pub struct LobbyFilter {
    pub game_mode: Option<GameMode>,
    pub map: Option<String>,
    pub has_password: Option<bool>,
    pub has_slots: bool,
    pub region: Option<String>,
}

impl LobbyFilter {
    fn matches(&self, lobby: &GameLobby) -> bool {
        if let Some(mode) = self.game_mode {
            if lobby.game_mode != mode {
                return false;
            }
        }

        if let Some(ref map) = self.map {
            if lobby.map != *map {
                return false;
            }
        }

        if let Some(has_password) = self.has_password {
            if lobby.is_password_protected != has_password {
                return false;
            }
        }

        if self.has_slots && lobby.players.len() >= lobby.max_players {
            return false;
        }

        if let Some(ref region) = self.region {
            if lobby.region != *region {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_matchmaking_service_creation() {
        let service = MatchmakingService::new();
        assert_eq!(service.get_online_players_count().await, 0);
    }

    #[tokio::test]
    async fn test_player_registration() {
        let service = MatchmakingService::new();

        let player = MatchmakingPlayer {
            player_id: Uuid::new_v4(),
            display_name: "TestPlayer".to_string(),
            rank: PlayerRank {
                tier: RankTier::Silver,
                division: 3,
                points: 1500,
            },
            skill_rating: 1200,
            preferred_faction: Some("USA".to_string()),
            status: PlayerStatus::Online,
            last_activity: Utc::now(),
            stats: PlayerStats::default(),
        };

        service.register_player(player.clone()).await.unwrap();
        assert_eq!(service.get_online_players_count().await, 1);

        service.unregister_player(player.player_id).await.unwrap();
        assert_eq!(service.get_online_players_count().await, 0);
    }

    #[tokio::test]
    async fn test_lobby_creation() {
        let service = MatchmakingService::new();

        let host_id = Uuid::new_v4();
        let player = MatchmakingPlayer {
            player_id: host_id,
            display_name: "Host".to_string(),
            rank: PlayerRank {
                tier: RankTier::Gold,
                division: 2,
                points: 2000,
            },
            skill_rating: 1500,
            preferred_faction: None,
            status: PlayerStatus::Online,
            last_activity: Utc::now(),
            stats: PlayerStats::default(),
        };

        service.register_player(player).await.unwrap();

        let lobby_id = service
            .create_lobby(
                host_id,
                "Test Lobby".to_string(),
                GameMode::Multiplayer,
                "Tournament Desert".to_string(),
                LobbySettings::default(),
                4,
                None,
            )
            .await
            .unwrap();

        let lobbies = service.get_lobbies(LobbyFilter::default()).await;
        assert_eq!(lobbies.len(), 1);
        assert_eq!(lobbies[0].lobby_id, lobby_id);
        assert_eq!(lobbies[0].players.len(), 1);
    }

    #[test]
    fn test_player_stats_calculation() {
        let mut stats = PlayerStats {
            games_played: 10,
            games_won: 7,
            games_lost: 3,
            ..Default::default()
        };

        stats.calculate_win_rate();
        assert_eq!(stats.win_rate, 70.0);
    }

    #[test]
    fn test_rank_tier_display() {
        assert_eq!(RankTier::Bronze.to_string(), "Bronze");
        assert_eq!(RankTier::Diamond.to_string(), "Diamond");
        assert_eq!(RankTier::Grandmaster.to_string(), "Grandmaster");
    }

    #[test]
    fn test_lobby_filter() {
        let lobby = GameLobby {
            lobby_id: Uuid::new_v4(),
            name: "Test".to_string(),
            host_player: Uuid::new_v4(),
            players: Vec::new(),
            max_players: 4,
            game_mode: GameMode::Multiplayer,
            map: "Test Map".to_string(),
            settings: LobbySettings::default(),
            status: LobbyStatus::WaitingForPlayers,
            created_at: Utc::now(),
            is_password_protected: false,
            spectator_slots: 2,
            spectators: Vec::new(),
            region: "US-East".to_string(),
        };

        let filter = LobbyFilter {
            game_mode: Some(GameMode::Multiplayer),
            has_password: Some(false),
            ..Default::default()
        };

        assert!(filter.matches(&lobby));

        let filter_no_match = LobbyFilter {
            game_mode: Some(GameMode::Tournament),
            ..Default::default()
        };

        assert!(!filter_no_match.matches(&lobby));
    }
}
