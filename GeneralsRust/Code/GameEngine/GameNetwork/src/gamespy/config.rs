#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy Configuration Management
//!
//! This module handles GameSpy configuration including:
//! - Server lists and connection settings
//! - Ping servers and latency measurement
//! - NAT traversal configuration
//! - Custom match restrictions
//! - Player VIP status and permissions

use super::chat_transport::ChatTransportConfig;
use crate::error::{NetworkError, NetworkResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use url::Url;

/// GameSpy configuration
pub struct GameSpyConfig {
    /// Ping servers for latency testing
    ping_servers: Vec<String>,
    /// Number of ping repetitions
    ping_repetitions: i32,
    /// Ping timeout in milliseconds
    ping_timeout_ms: i32,
    /// Good ping cutoff (milliseconds)
    ping_cutoff_good: i32,
    /// Bad ping cutoff (milliseconds)
    ping_cutoff_bad: i32,

    /// NAT traversal settings
    nat_retry_interval: i32,
    nat_max_mangler_retries: i32,
    nat_mangler_retry_interval: i32,
    nat_keepalive_interval: i32,
    nat_port_timeout: i32,
    nat_round_timeout: i32,

    /// Custom match settings
    restrict_games_to_lobby: bool,

    /// Quick match maps
    qm_maps: Vec<String>,
    qm_bot_id: i32,
    qm_channel: i32,

    /// Player permissions
    vip_players: HashMap<String, PlayerPermissions>,

    /// Server endpoints
    master_server_url: String,
    chat_server_url: String,
    chat_auth_token: Option<String>,
    ladder_server_url: String,

    /// Connection settings
    connection_timeout_seconds: u64,
    max_reconnect_attempts: u32,

    /// File path for config storage
    config_file_path: String,
    /// Directory for persistent storage
    storage_directory: String,
    /// Raw config contents for ladder parsing
    leftover_config: String,
}

/// Player permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPermissions {
    /// Player is VIP
    pub is_vip: bool,
    /// Player rank for ranking system
    pub rank: i32,
    /// Special permissions
    pub can_create_tournaments: bool,
    pub can_moderate_chat: bool,
    pub can_host_ladder_games: bool,
}

/// NAT location information
#[derive(Debug, Clone)]
pub struct NatLocation {
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
}

impl Default for GameSpyConfig {
    fn default() -> Self {
        Self {
            ping_servers: vec![
                "motd.gamespy.com".to_string(),
                "ccgenerals.ms5.gamespy.com".to_string(),
                "ccgenerals.ms6.gamespy.com".to_string(),
            ],
            ping_repetitions: 5,
            ping_timeout_ms: 2000,
            ping_cutoff_good: 150,
            ping_cutoff_bad: 300,

            nat_retry_interval: 1000,
            nat_max_mangler_retries: 5,
            nat_mangler_retry_interval: 5000,
            nat_keepalive_interval: 60000,
            nat_port_timeout: 30000,
            nat_round_timeout: 120000,

            restrict_games_to_lobby: false,

            qm_maps: vec![
                "Tournament Desert".to_string(),
                "Tournament Snow".to_string(),
                "Tournament Urban".to_string(),
            ],
            qm_bot_id: 0,
            qm_channel: 0,

            vip_players: HashMap::new(),

            master_server_url: "master.gamespy.com".to_string(),
            chat_server_url: "wss://chat.gamespy.com/ws".to_string(),
            chat_auth_token: None,
            ladder_server_url: "ladder.gamespy.com".to_string(),

            connection_timeout_seconds: 30,
            max_reconnect_attempts: 3,

            config_file_path: "gamespy_config.ini".to_string(),
            storage_directory: "storage/gamespy".to_string(),
            leftover_config: String::new(),
        }
    }
}

impl GameSpyConfig {
    /// Create new GameSpy config
    pub async fn new() -> NetworkResult<Self> {
        let mut config = Self::default();

        // Try to load from file
        if let Err(e) = config.load_from_file().await {
            warn!("Failed to load GameSpy config from file: {}", e);
            // Continue with defaults
        }

        Ok(config)
    }

    /// Create a new config instance synchronously.
    pub fn new_sync() -> Self {
        let mut config = Self::default();
        if let Err(e) = config.load_from_file_sync() {
            warn!("Failed to load GameSpy config from file: {}", e);
        }
        config
    }

    /// Load configuration from file
    #[instrument(skip(self))]
    pub async fn load_from_file(&mut self) -> NetworkResult<()> {
        let path = Path::new(&self.config_file_path);

        if !path.exists() {
            info!("GameSpy config file doesn't exist, using defaults");
            return Ok(());
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| NetworkError::generic(format!("Failed to read config file: {}", e)))?;

        // Parse INI-style configuration
        self.parse_config_contents(&contents)?;

        info!("Loaded GameSpy config from {}", self.config_file_path);
        Ok(())
    }

    /// Load configuration from file synchronously.
    pub fn load_from_file_sync(&mut self) -> NetworkResult<()> {
        let path = Path::new(&self.config_file_path);
        if !path.exists() {
            info!("GameSpy config file doesn't exist, using defaults");
            return Ok(());
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| NetworkError::generic(format!("Failed to read config file: {}", e)))?;
        self.parse_config_contents(&contents)?;
        Ok(())
    }

    /// Save configuration to file
    #[instrument(skip(self))]
    pub async fn save_to_file(&self) -> NetworkResult<()> {
        let contents = self.generate_config_contents();

        fs::write(&self.config_file_path, contents)
            .map_err(|e| NetworkError::generic(format!("Failed to write config file: {}", e)))?;

        info!("Saved GameSpy config to {}", self.config_file_path);
        Ok(())
    }

    /// Parse configuration contents
    fn parse_config_contents(&mut self, contents: &str) -> NetworkResult<()> {
        self.leftover_config = contents.to_string();
        let mut current_section = String::new();

        for line in contents.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue; // Skip comments and empty lines
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match current_section.as_str() {
                    "PING" => self.parse_ping_setting(key, value)?,
                    "NAT" => self.parse_nat_setting(key, value)?,
                    "CUSTOM_MATCH" => self.parse_custom_match_setting(key, value)?,
                    "QM" => self.parse_qm_setting(key, value)?,
                    "SERVERS" => self.parse_server_setting(key, value)?,
                    "CONNECTION" => self.parse_connection_setting(key, value)?,
                    "CHAT" => self.parse_chat_setting(key, value)?,
                    "STORAGE" => self.parse_storage_setting(key, value)?,
                    "VIP" => self.parse_vip_setting(key, value)?,
                    _ => {
                        warn!("Unknown config section: {}", current_section);
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse ping settings
    fn parse_ping_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "Servers" => {
                self.ping_servers = value.split(',').map(|s| s.trim().to_string()).collect();
            }
            "Repetitions" => {
                self.ping_repetitions = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid ping repetitions value"))?;
            }
            "Timeout" => {
                self.ping_timeout_ms = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid ping timeout value"))?;
            }
            "CutoffGood" => {
                self.ping_cutoff_good = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid ping cutoff good value"))?;
            }
            "CutoffBad" => {
                self.ping_cutoff_bad = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid ping cutoff bad value"))?;
            }
            _ => {
                warn!("Unknown ping setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse NAT settings
    fn parse_nat_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "RetryInterval" => {
                self.nat_retry_interval = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT retry interval"))?;
            }
            "MaxManglerRetries" => {
                self.nat_max_mangler_retries = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT max mangler retries"))?;
            }
            "ManglerRetryInterval" => {
                self.nat_mangler_retry_interval = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT mangler retry interval"))?;
            }
            "KeepaliveInterval" => {
                self.nat_keepalive_interval = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT keepalive interval"))?;
            }
            "PortTimeout" => {
                self.nat_port_timeout = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT port timeout"))?;
            }
            "RoundTimeout" => {
                self.nat_round_timeout = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid NAT round timeout"))?;
            }
            _ => {
                warn!("Unknown NAT setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse custom match settings
    fn parse_custom_match_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "RestrictGamesToLobby" => {
                self.restrict_games_to_lobby = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid restrict games to lobby value"))?;
            }
            _ => {
                warn!("Unknown custom match setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse QM settings
    fn parse_qm_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "Maps" => {
                self.qm_maps = value.split(',').map(|s| s.trim().to_string()).collect();
            }
            "BotID" => {
                self.qm_bot_id = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid QM bot ID"))?;
            }
            "Channel" => {
                self.qm_channel = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid QM channel"))?;
            }
            _ => {
                warn!("Unknown QM setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse server settings
    fn parse_server_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "MasterServer" => {
                self.master_server_url = value.to_string();
            }
            "ChatServer" => {
                self.chat_server_url = value.to_string();
            }
            "LadderServer" => {
                self.ladder_server_url = value.to_string();
            }
            _ => {
                warn!("Unknown server setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse connection settings
    fn parse_connection_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "Timeout" => {
                self.connection_timeout_seconds = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid connection timeout"))?;
            }
            "MaxReconnectAttempts" => {
                self.max_reconnect_attempts = value
                    .parse()
                    .map_err(|_| NetworkError::generic("Invalid max reconnect attempts"))?;
            }
            _ => {
                warn!("Unknown connection setting: {}", key);
            }
        }
        Ok(())
    }

    /// Parse chat settings
    fn parse_chat_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "ServerUrl" => {
                self.chat_server_url = value.to_string();
            }
            "AuthToken" => {
                self.chat_auth_token = if value.trim().is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            _ => warn!("Unknown chat setting: {}", key),
        }
        Ok(())
    }

    /// Parse VIP settings
    fn parse_vip_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        let permissions: PlayerPermissions = serde_json::from_str(value)
            .map_err(|_| NetworkError::generic(format!("Invalid VIP permissions for {}", key)))?;

        self.vip_players.insert(key.to_string(), permissions);
        Ok(())
    }

    /// Parse storage settings
    fn parse_storage_setting(&mut self, key: &str, value: &str) -> NetworkResult<()> {
        match key {
            "Directory" => {
                self.storage_directory = value.to_string();
            }
            _ => warn!("Unknown storage setting: {}", key),
        }
        Ok(())
    }

    /// Chat server endpoint (WebSocket URL) used for GameSpy chat.
    pub fn chat_server_endpoint(&self) -> &str {
        &self.chat_server_url
    }

    /// Update the chat authentication token used for backend connections.
    pub fn set_chat_auth_token(&mut self, token: Option<String>) {
        self.chat_auth_token = token;
    }

    /// Build a chat transport configuration ready for establishing WebSocket connections.
    pub fn chat_transport_config(&self) -> NetworkResult<ChatTransportConfig> {
        let url = Url::parse(&self.chat_server_url).map_err(|err| {
            NetworkError::generic(format!(
                "Invalid chat server URL '{}': {}",
                self.chat_server_url, err
            ))
        })?;

        let mut config = ChatTransportConfig::new(url);
        if let Some(token) = &self.chat_auth_token {
            if !token.trim().is_empty() {
                config = config.with_auth_token(token.clone());
            }
        }

        Ok(config)
    }

    /// Directory used for persisting ladder, buddy, and stats data.
    pub fn storage_directory(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.storage_directory)
    }

    /// Generate configuration contents
    fn generate_config_contents(&self) -> String {
        let mut contents = String::new();

        // PING section
        contents.push_str("[PING]\n");
        contents.push_str(&format!("Servers={}\n", self.ping_servers.join(",")));
        contents.push_str(&format!("Repetitions={}\n", self.ping_repetitions));
        contents.push_str(&format!("Timeout={}\n", self.ping_timeout_ms));
        contents.push_str(&format!("CutoffGood={}\n", self.ping_cutoff_good));
        contents.push_str(&format!("CutoffBad={}\n", self.ping_cutoff_bad));
        contents.push_str("\n");

        // NAT section
        contents.push_str("[NAT]\n");
        contents.push_str(&format!("RetryInterval={}\n", self.nat_retry_interval));
        contents.push_str(&format!(
            "MaxManglerRetries={}\n",
            self.nat_max_mangler_retries
        ));
        contents.push_str(&format!(
            "ManglerRetryInterval={}\n",
            self.nat_mangler_retry_interval
        ));
        contents.push_str(&format!(
            "KeepaliveInterval={}\n",
            self.nat_keepalive_interval
        ));
        contents.push_str(&format!("PortTimeout={}\n", self.nat_port_timeout));
        contents.push_str(&format!("RoundTimeout={}\n", self.nat_round_timeout));
        contents.push_str("\n");

        // CUSTOM_MATCH section
        contents.push_str("[CUSTOM_MATCH]\n");
        contents.push_str(&format!(
            "RestrictGamesToLobby={}\n",
            self.restrict_games_to_lobby
        ));
        contents.push_str("\n");

        // QM section
        contents.push_str("[QM]\n");
        contents.push_str(&format!("Maps={}\n", self.qm_maps.join(",")));
        contents.push_str(&format!("BotID={}\n", self.qm_bot_id));
        contents.push_str(&format!("Channel={}\n", self.qm_channel));
        contents.push_str("\n");

        // SERVERS section
        contents.push_str("[SERVERS]\n");
        contents.push_str(&format!("MasterServer={}\n", self.master_server_url));
        contents.push_str(&format!("ChatServer={}\n", self.chat_server_url));
        contents.push_str(&format!("LadderServer={}\n", self.ladder_server_url));
        contents.push_str("\n");

        // CHAT section
        contents.push_str("[CHAT]\n");
        contents.push_str(&format!("ServerUrl={}\n", self.chat_server_url));
        if let Some(token) = &self.chat_auth_token {
            contents.push_str(&format!("AuthToken={}\n", token));
        }
        contents.push_str("\n");

        // STORAGE section
        contents.push_str("[STORAGE]\n");
        contents.push_str(&format!(
            "Directory={}\n",
            self.storage_directory.replace('\\', "/")
        ));
        contents.push_str("\n");

        // CONNECTION section
        contents.push_str("[CONNECTION]\n");
        contents.push_str(&format!("Timeout={}\n", self.connection_timeout_seconds));
        contents.push_str(&format!(
            "MaxReconnectAttempts={}\n",
            self.max_reconnect_attempts
        ));
        contents.push_str("\n");

        // VIP section
        if !self.vip_players.is_empty() {
            contents.push_str("[VIP]\n");
            for (player_id, permissions) in &self.vip_players {
                if let Ok(json) = serde_json::to_string(permissions) {
                    contents.push_str(&format!("{}={}\n", player_id, json));
                }
            }
            contents.push_str("\n");
        }

        contents
    }

    /// Get ping servers
    pub fn get_ping_servers(&self) -> &[String] {
        &self.ping_servers
    }

    /// Get ping configuration
    pub fn get_ping_config(&self) -> (i32, i32, i32, i32) {
        (
            self.ping_repetitions,
            self.ping_timeout_ms,
            self.ping_cutoff_good,
            self.ping_cutoff_bad,
        )
    }

    /// Get QM maps
    pub fn get_qm_maps(&self) -> &[String] {
        &self.qm_maps
    }

    pub fn leftover_config(&self) -> &str {
        &self.leftover_config
    }

    /// Get QM configuration
    pub fn get_qm_config(&self) -> (i32, i32) {
        (self.qm_bot_id, self.qm_channel)
    }

    /// Set QM channel
    pub fn set_qm_channel(&mut self, channel: i32) {
        self.qm_channel = channel;
    }

    /// Check if player is VIP
    pub fn is_player_vip(&self, player_id: &str) -> bool {
        self.vip_players
            .get(player_id)
            .map(|p| p.is_vip)
            .unwrap_or(false)
    }

    /// Get player permissions
    pub fn get_player_permissions(&self, player_id: &str) -> Option<&PlayerPermissions> {
        self.vip_players.get(player_id)
    }

    /// Add VIP player
    pub fn add_vip_player(&mut self, player_id: String, permissions: PlayerPermissions) {
        self.vip_players.insert(player_id, permissions);
    }

    /// Remove VIP player
    pub fn remove_vip_player(&mut self, player_id: &str) {
        self.vip_players.remove(player_id);
    }

    /// Get NAT location
    pub fn get_nat_location(&self, index: usize) -> Option<NatLocation> {
        if index < self.ping_servers.len() {
            Some(NatLocation {
                host: self.ping_servers[index].clone(),
                port: 27900, // Default GameSpy port
            })
        } else {
            None
        }
    }

    /// Get NAT configuration
    pub fn get_nat_config(&self) -> (i32, i32, i32, i32, i32, i32) {
        (
            self.nat_retry_interval,
            self.nat_max_mangler_retries,
            self.nat_mangler_retry_interval,
            self.nat_keepalive_interval,
            self.nat_port_timeout,
            self.nat_round_timeout,
        )
    }

    /// Check if games should be restricted to lobby
    pub fn restrict_games_to_lobby(&self) -> bool {
        self.restrict_games_to_lobby
    }

    /// Get server URLs
    pub fn get_server_urls(&self) -> (&str, &str, &str) {
        (
            &self.master_server_url,
            &self.chat_server_url,
            &self.ladder_server_url,
        )
    }

    /// Get connection configuration
    pub fn get_connection_config(&self) -> (u64, u32) {
        (self.connection_timeout_seconds, self.max_reconnect_attempts)
    }

    /// Validate credentials
    pub fn validate_credentials(&self, player_id: &str, password: &str) -> NetworkResult<()> {
        // In a real implementation, this would validate against GameSpy servers
        // For now, just basic validation
        if player_id.is_empty() {
            return Err(NetworkError::generic("Player ID cannot be empty"));
        }

        if password.len() < 3 {
            return Err(NetworkError::generic("Password too short"));
        }

        Ok(())
    }

    /// Get points for rank
    pub fn get_points_for_rank(&self, rank: i32) -> i32 {
        const RANK_POINTS: [i32; 10] = [0, 5, 10, 20, 50, 100, 200, 500, 1000, 2000];
        let mut idx = rank;
        if idx < 0 {
            idx = 0;
        }
        if idx as usize >= RANK_POINTS.len() {
            idx = (RANK_POINTS.len() - 1) as i32;
        }
        RANK_POINTS[idx as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_creation() {
        let config = GameSpyConfig::new().await.unwrap();
        assert!(!config.get_ping_servers().is_empty());
        assert!(config.get_qm_maps().len() > 0);
    }

    #[test]
    fn test_config_defaults() {
        let config = GameSpyConfig::default();
        assert_eq!(config.ping_repetitions, 5);
        assert_eq!(config.ping_timeout_ms, 2000);
        assert!(!config.restrict_games_to_lobby);
    }

    #[test]
    fn test_vip_management() {
        let mut config = GameSpyConfig::default();

        let permissions = PlayerPermissions {
            is_vip: true,
            rank: 5,
            can_create_tournaments: true,
            can_moderate_chat: false,
            can_host_ladder_games: true,
        };

        config.add_vip_player("test_player".to_string(), permissions.clone());
        assert!(config.is_player_vip("test_player"));

        let retrieved = config.get_player_permissions("test_player").unwrap();
        assert_eq!(retrieved.rank, 5);

        config.remove_vip_player("test_player");
        assert!(!config.is_player_vip("test_player"));
    }

    #[test]
    fn test_rank_points() {
        let config = GameSpyConfig::default();
        assert_eq!(config.get_points_for_rank(0), 0);
        assert_eq!(config.get_points_for_rank(1), 5);
        assert_eq!(config.get_points_for_rank(9), 2000);
    }

    #[test]
    fn test_nat_location() {
        let config = GameSpyConfig::default();
        let location = config.get_nat_location(0).unwrap();
        assert_eq!(location.port, 27900);
        assert!(location.host.contains("gamespy.com"));
    }

    #[tokio::test]
    async fn test_chat_transport_config_with_token() {
        let mut config = GameSpyConfig::default();
        config.chat_server_url = "ws://localhost:1234".to_string();
        config.set_chat_auth_token(Some("secret-token".into()));

        let transport = config.chat_transport_config().unwrap();
        assert_eq!(transport.endpoint.as_str(), "ws://localhost:1234/");
        assert_eq!(transport.auth_token.as_deref(), Some("secret-token"));
    }
}
