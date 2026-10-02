//! Game browser for LAN and online game discovery
//!
//! This module provides a comprehensive game browser interface that combines
//! LAN discovery and online matchmaking into a unified game listing interface.

use crate::error::{NetworkError, NetworkResult};
use crate::lan_api::{GameOptions, LanGameInfo};
use crate::matchmaking::{GameLobby, LobbyFilter, MatchmakingService};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

/// Game browser sorting options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameSortType {
    /// Sort by name alphabetically ascending
    AlphaAscending,
    /// Sort by name alphabetically descending
    AlphaDescending,
    /// Sort by ping ascending (lowest first)
    PingAscending,
    /// Sort by ping descending (highest first)
    PingDescending,
    /// Sort by player count (most players first)
    PlayerCountDescending,
    /// Sort by player count (least players first)
    PlayerCountAscending,
    /// Sort by creation time (newest first)
    CreatedRecent,
    /// Sort by creation time (oldest first)
    CreatedOldest,
}

impl Default for GameSortType {
    fn default() -> Self {
        Self::AlphaAscending
    }
}

/// Game browser filter criteria
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameFilter {
    /// Filter by game name (partial match)
    pub name_filter: Option<String>,
    /// Filter by map name
    pub map_filter: Option<String>,
    /// Show only games with available slots
    pub has_slots: bool,
    /// Show only password-protected games
    pub password_protected: Option<bool>,
    /// Show only games with friends/buddies
    pub has_friends: bool,
    /// Filter by minimum ping (ms)
    pub min_ping: Option<u32>,
    /// Filter by maximum ping (ms)
    pub max_ping: Option<u32>,
    /// Show only games that haven't started
    pub not_started_only: bool,
    /// Filter by specific host IP
    pub host_ip_filter: Option<IpAddr>,
    /// Show only ranked games
    pub ranked_only: Option<bool>,
}

/// Information about a game in the browser
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserGame {
    /// Unique game identifier
    pub game_id: Uuid,
    /// Game name
    pub name: String,
    /// Host player name
    pub host_name: String,
    /// Host IP address
    pub host_ip: IpAddr,
    /// Game port
    pub port: u16,
    /// Map name
    pub map_name: String,
    /// Current player count
    pub player_count: u8,
    /// Maximum players
    pub max_players: u8,
    /// Has password protection
    pub has_password: bool,
    /// Game has started
    pub has_started: bool,
    /// Is ranked game
    pub is_ranked: bool,
    /// Ping to host (milliseconds)
    pub ping_ms: Option<u32>,
    /// Game creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub last_seen: DateTime<Utc>,
    /// Game options
    pub options: GameOptions,
    /// Is LAN game (vs online matchmaking)
    pub is_lan: bool,
    /// Version hash for compatibility
    pub version_hash: u32,
    /// Map CRC for validation
    pub map_crc: Option<u32>,
    /// Friends in this game
    pub friends_in_game: Vec<String>,
}

impl BrowserGame {
    /// Create from LAN game info
    pub fn from_lan_game(game: &LanGameInfo) -> Self {
        let host_name = game
            .get_host()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            game_id: game.game_id,
            name: game.name.clone(),
            host_name,
            host_ip: game.host_ip,
            port: game.port,
            map_name: game.options.map_name.clone(),
            player_count: game.player_count,
            max_players: game.max_players,
            has_password: game.has_password,
            has_started: game.has_started(),
            is_ranked: false, // LAN games are not ranked
            ping_ms: None,    // Would be populated by ping service
            created_at: game.created_at,
            last_seen: game.last_heard,
            options: game.options.clone(),
            is_lan: true,
            version_hash: game.version_hash,
            map_crc: game.map_crc,
            friends_in_game: Vec::new(), // Would be populated by friend service
        }
    }

    /// Create from matchmaking lobby
    pub fn from_matchmaking_lobby(lobby: &GameLobby) -> Self {
        let host_name = lobby
            .players
            .iter()
            .find(|p| p.player_id == lobby.host_player)
            .map(|p| p.display_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            game_id: lobby.lobby_id,
            name: lobby.name.clone(),
            host_name,
            host_ip: IpAddr::from([0, 0, 0, 0]), // Online games don't have direct IP
            port: 0,
            map_name: lobby.map.clone(),
            player_count: lobby.players.len() as u8,
            max_players: lobby.max_players as u8,
            has_password: lobby.is_password_protected,
            has_started: matches!(lobby.status, crate::matchmaking::LobbyStatus::InProgress),
            is_ranked: lobby.settings.is_ranked,
            ping_ms: None,
            created_at: lobby.created_at,
            last_seen: Utc::now(),
            options: GameOptions {
                map_name: lobby.map.clone(),
                seed: 0, // Would be set by lobby
                speed: lobby.settings.game_speed,
                starting_money: crate::lan_api::game_info::StartingMoney::Normal, // Map from settings
                difficulty: crate::lan_api::game_info::GameDifficulty::Normal,
                victory_condition: crate::lan_api::game_info::VictoryCondition::Annihilation,
                time_limit: 0,
                superweapons_enabled: lobby.settings.allow_superweapons,
                crates_enabled: false,
                fog_of_war: true,
                custom_rules: HashMap::new(),
            },
            is_lan: false,
            version_hash: 0,
            map_crc: None,
            friends_in_game: Vec::new(),
        }
    }

    /// Check if game matches filter criteria
    pub fn matches_filter(&self, filter: &GameFilter, friend_list: &[String]) -> bool {
        // Name filter
        if let Some(ref name) = filter.name_filter {
            if !self.name.to_lowercase().contains(&name.to_lowercase()) {
                return false;
            }
        }

        // Map filter
        if let Some(ref map) = filter.map_filter {
            if !self.map_name.to_lowercase().contains(&map.to_lowercase()) {
                return false;
            }
        }

        // Has slots filter
        if filter.has_slots && self.player_count >= self.max_players {
            return false;
        }

        // Password filter
        if let Some(password_protected) = filter.password_protected {
            if self.has_password != password_protected {
                return false;
            }
        }

        // Friends filter
        if filter.has_friends {
            let has_any_friend = self
                .friends_in_game
                .iter()
                .any(|friend| friend_list.contains(friend));
            if !has_any_friend {
                return false;
            }
        }

        // Ping filter
        if let Some(ping) = self.ping_ms {
            if let Some(min_ping) = filter.min_ping {
                if ping < min_ping {
                    return false;
                }
            }
            if let Some(max_ping) = filter.max_ping {
                if ping > max_ping {
                    return false;
                }
            }
        }

        // Not started filter
        if filter.not_started_only && self.has_started {
            return false;
        }

        // Host IP filter
        if let Some(host_ip) = filter.host_ip_filter {
            if self.host_ip != host_ip {
                return false;
            }
        }

        // Ranked filter
        if let Some(ranked) = filter.ranked_only {
            if self.is_ranked != ranked {
                return false;
            }
        }

        true
    }

    /// Get display string for game listing
    pub fn get_display_string(&self) -> String {
        let status = if self.has_started {
            "In Progress"
        } else {
            "Waiting"
        };

        let password_marker = if self.has_password { "[P]" } else { "" };
        let ranked_marker = if self.is_ranked { "[R]" } else { "" };
        let ping_str = self
            .ping_ms
            .map(|p| format!("{}ms", p))
            .unwrap_or_else(|| "?".to_string());

        format!(
            "{}{}{} - {}/{} - {} - {} - {}",
            self.name,
            password_marker,
            ranked_marker,
            self.player_count,
            self.max_players,
            self.map_name,
            status,
            ping_str
        )
    }
}

/// Game browser for discovering and filtering games
///
/// Single-owner state: the game tables, the filter/sort knobs and the refresh
/// stamp are only touched from this struct's own methods (mutators take
/// `&mut self`), so they are plain fields.  Only the matchmaking service is
/// shared, and it arrives pre-wrapped in an `Arc`.
pub struct GameBrowser {
    /// LAN games discovered
    lan_games: HashMap<Uuid, BrowserGame>,

    /// Online matchmaking games
    online_games: HashMap<Uuid, BrowserGame>,

    /// Current filter settings
    filter: GameFilter,

    /// Current sort type
    sort_type: GameSortType,

    /// Friend list for filtering
    friend_list: Vec<String>,

    /// Sort by friends first
    sort_friends_first: bool,

    /// Reference to matchmaking service (optional)
    matchmaking_service: Option<Arc<MatchmakingService>>,

    /// Game refresh interval
    refresh_interval: std::time::Duration,

    /// Last refresh time
    last_refresh: std::time::Instant,
}

impl GameBrowser {
    /// Create new game browser
    pub fn new() -> Self {
        Self {
            lan_games: HashMap::new(),
            online_games: HashMap::new(),
            filter: GameFilter::default(),
            sort_type: GameSortType::default(),
            friend_list: Vec::new(),
            sort_friends_first: true,
            matchmaking_service: None,
            refresh_interval: std::time::Duration::from_secs(5),
            last_refresh: std::time::Instant::now(),
        }
    }

    /// Create with matchmaking service
    pub fn with_matchmaking(matchmaking: Arc<MatchmakingService>) -> Self {
        let mut browser = Self::new();
        browser.matchmaking_service = Some(matchmaking);
        browser
    }

    /// Update LAN games list
    pub async fn update_lan_games(&mut self, games: Vec<LanGameInfo>) {
        // Convert to browser games
        for game in games {
            let browser_game = BrowserGame::from_lan_game(&game);
            self.lan_games.insert(game.game_id, browser_game);
        }

        // Remove stale games (not seen in last update)
        let now = Utc::now();
        let stale_timeout = chrono::Duration::seconds(30);
        self.lan_games
            .retain(|_, game| now.signed_duration_since(game.last_seen) < stale_timeout);

        debug!("Updated LAN games list: {} games", self.lan_games.len());
    }

    /// Update ping for a game
    pub async fn update_game_ping(&mut self, game_id: Uuid, ping_ms: u32) {
        if let Some(game) = self.lan_games.get_mut(&game_id) {
            game.ping_ms = Some(ping_ms);
        } else if let Some(game) = self.online_games.get_mut(&game_id) {
            game.ping_ms = Some(ping_ms);
        }
    }

    /// Update friends in a game
    pub async fn update_game_friends(&mut self, game_id: Uuid, friends: Vec<String>) {
        if let Some(game) = self.lan_games.get_mut(&game_id) {
            game.friends_in_game = friends;
        } else if let Some(game) = self.online_games.get_mut(&game_id) {
            game.friends_in_game = friends;
        }
    }

    /// Refresh online games from matchmaking service
    pub async fn refresh_online_games(&mut self) -> NetworkResult<()> {
        let matchmaking = self.matchmaking_service.clone();

        if let Some(matchmaking) = matchmaking {
            let lobbies = matchmaking.get_lobbies(LobbyFilter::default()).await;

            self.online_games.clear();

            for lobby in lobbies {
                let browser_game = BrowserGame::from_matchmaking_lobby(&lobby);
                self.online_games.insert(lobby.lobby_id, browser_game);
            }

            debug!("Refreshed online games: {} games", self.online_games.len());
        }

        self.last_refresh = std::time::Instant::now();
        Ok(())
    }

    /// Get filtered and sorted game list
    pub async fn get_games(&self) -> Vec<BrowserGame> {
        let filter = self.filter.clone();
        let sort_type = self.sort_type;
        let friend_list = self.friend_list.clone();
        let sort_friends_first = self.sort_friends_first;

        // Combine LAN and online games
        let mut games: Vec<BrowserGame> = self
            .lan_games
            .values()
            .chain(self.online_games.values())
            .filter(|game| game.matches_filter(&filter, &friend_list))
            .cloned()
            .collect();

        // Sort games
        Self::sort_games(&mut games, sort_type, &friend_list, sort_friends_first);

        games
    }

    /// Sort games by criteria
    fn sort_games(
        games: &mut [BrowserGame],
        sort_type: GameSortType,
        friend_list: &[String],
        friends_first: bool,
    ) {
        games.sort_by(|a, b| {
            // Always sort friends first if enabled
            if friends_first {
                let a_has_friends = a.friends_in_game.iter().any(|f| friend_list.contains(f));
                let b_has_friends = b.friends_in_game.iter().any(|f| friend_list.contains(f));

                if a_has_friends != b_has_friends {
                    return b_has_friends.cmp(&a_has_friends);
                }
            }

            // Then sort by selected criteria
            match sort_type {
                GameSortType::AlphaAscending => a.name.cmp(&b.name),
                GameSortType::AlphaDescending => b.name.cmp(&a.name),
                GameSortType::PingAscending => {
                    let a_ping = a.ping_ms.unwrap_or(9999);
                    let b_ping = b.ping_ms.unwrap_or(9999);
                    a_ping.cmp(&b_ping)
                }
                GameSortType::PingDescending => {
                    let a_ping = a.ping_ms.unwrap_or(0);
                    let b_ping = b.ping_ms.unwrap_or(0);
                    b_ping.cmp(&a_ping)
                }
                GameSortType::PlayerCountDescending => b.player_count.cmp(&a.player_count),
                GameSortType::PlayerCountAscending => a.player_count.cmp(&b.player_count),
                GameSortType::CreatedRecent => b.created_at.cmp(&a.created_at),
                GameSortType::CreatedOldest => a.created_at.cmp(&b.created_at),
            }
        });
    }

    /// Set filter
    pub async fn set_filter(&mut self, filter: GameFilter) {
        self.filter = filter;
        info!("Game browser filter updated");
    }

    /// Set sort type
    pub async fn set_sort_type(&mut self, sort_type: GameSortType) {
        self.sort_type = sort_type;
        info!("Game browser sort changed to {:?}", sort_type);
    }

    /// Set friend list for filtering
    pub async fn set_friend_list(&mut self, friends: Vec<String>) {
        self.friend_list = friends;
    }

    /// Set whether to sort friends first
    pub async fn set_sort_friends_first(&mut self, enabled: bool) {
        self.sort_friends_first = enabled;
    }

    /// Get current filter
    pub async fn get_filter(&self) -> GameFilter {
        self.filter.clone()
    }

    /// Get current sort type
    pub async fn get_sort_type(&self) -> GameSortType {
        self.sort_type
    }

    /// Clear all games
    pub async fn clear(&mut self) {
        self.lan_games.clear();
        self.online_games.clear();
        info!("Game browser cleared");
    }

    /// Get game by ID
    pub async fn get_game(&self, game_id: Uuid) -> Option<BrowserGame> {
        if let Some(game) = self.lan_games.get(&game_id) {
            return Some(game.clone());
        }

        if let Some(game) = self.online_games.get(&game_id) {
            return Some(game.clone());
        }

        None
    }

    /// Check if refresh is needed
    pub async fn needs_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= self.refresh_interval
    }

    /// Get game count
    pub async fn get_game_count(&self) -> usize {
        self.lan_games.len() + self.online_games.len()
    }
}

impl Default for GameBrowser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_game_browser_creation() {
        let browser = GameBrowser::new();
        assert_eq!(browser.get_game_count().await, 0);
    }

    #[tokio::test]
    async fn test_game_filter() {
        let game = BrowserGame {
            game_id: Uuid::new_v4(),
            name: "Test Game".to_string(),
            host_name: "Host".to_string(),
            host_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8087,
            map_name: "Tournament Desert".to_string(),
            player_count: 2,
            max_players: 8,
            has_password: false,
            has_started: false,
            is_ranked: false,
            ping_ms: Some(50),
            created_at: Utc::now(),
            last_seen: Utc::now(),
            options: GameOptions::default(),
            is_lan: true,
            version_hash: 0,
            map_crc: None,
            friends_in_game: vec!["Friend1".to_string()],
        };

        // Test name filter
        let filter = GameFilter {
            name_filter: Some("Test".to_string()),
            ..Default::default()
        };
        assert!(game.matches_filter(&filter, &[]));

        let filter = GameFilter {
            name_filter: Some("Other".to_string()),
            ..Default::default()
        };
        assert!(!game.matches_filter(&filter, &[]));

        // Test has slots filter
        let filter = GameFilter {
            has_slots: true,
            ..Default::default()
        };
        assert!(game.matches_filter(&filter, &[]));

        // Test ping filter
        let filter = GameFilter {
            max_ping: Some(100),
            ..Default::default()
        };
        assert!(game.matches_filter(&filter, &[]));

        let filter = GameFilter {
            max_ping: Some(30),
            ..Default::default()
        };
        assert!(!game.matches_filter(&filter, &[]));
    }

    #[tokio::test]
    async fn test_game_sorting() {
        let mut games = vec![
            BrowserGame {
                game_id: Uuid::new_v4(),
                name: "Zebra Game".to_string(),
                ping_ms: Some(100),
                player_count: 4,
                ..create_test_game()
            },
            BrowserGame {
                game_id: Uuid::new_v4(),
                name: "Alpha Game".to_string(),
                ping_ms: Some(50),
                player_count: 2,
                ..create_test_game()
            },
        ];

        // Test alpha sort
        GameBrowser::sort_games(&mut games, GameSortType::AlphaAscending, &[], false);
        assert_eq!(games[0].name, "Alpha Game");

        GameBrowser::sort_games(&mut games, GameSortType::AlphaDescending, &[], false);
        assert_eq!(games[0].name, "Zebra Game");

        // Test ping sort
        GameBrowser::sort_games(&mut games, GameSortType::PingAscending, &[], false);
        assert_eq!(games[0].ping_ms, Some(50));

        // Test player count sort
        GameBrowser::sort_games(&mut games, GameSortType::PlayerCountDescending, &[], false);
        assert_eq!(games[0].player_count, 4);
    }

    fn create_test_game() -> BrowserGame {
        BrowserGame {
            game_id: Uuid::new_v4(),
            name: "Test".to_string(),
            host_name: "Host".to_string(),
            host_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8087,
            map_name: "Test Map".to_string(),
            player_count: 1,
            max_players: 8,
            has_password: false,
            has_started: false,
            is_ranked: false,
            ping_ms: None,
            created_at: Utc::now(),
            last_seen: Utc::now(),
            options: GameOptions::default(),
            is_lan: true,
            version_hash: 0,
            map_crc: None,
            friends_in_game: Vec::new(),
        }
    }
}
