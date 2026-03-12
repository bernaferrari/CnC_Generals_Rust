//! LAN chat system for in-game communication
//!
//! This module provides chat functionality for LAN games, supporting different
//! chat types and message broadcasting to players.

use crate::connection::ConnectionManager;
use crate::error::{NetworkError, NetworkResult};
use crate::lan_api::crypto::LanCrypto;
use crate::lan_api::{LanBridgeEvent, LanEventSender};
use crate::security::SecurityManager;
use crate::time::NetworkInstant;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::net::UdpSocket;
use tokio::sync::{Notify, RwLock};
use tokio::time::interval;
use tracing::{debug, info, trace, warn};
use uuid::Uuid;

/// Chat message types matching the original C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    /// Normal chat message
    Normal = 0,
    /// Emote/action message (e.g., "/me does something")
    Emote = 1,
    /// System message (game notifications, etc.)
    System = 2,
}

impl Default for ChatType {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for ChatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatType::Normal => write!(f, "Normal"),
            ChatType::Emote => write!(f, "Emote"),
            ChatType::System => write!(f, "System"),
        }
    }
}

/// A chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: Uuid,
    /// Sender's display name
    pub sender_name: String,
    /// Sender's IP address
    pub sender_ip: IpAddr,
    /// Message content
    pub message: String,
    /// Message type
    pub chat_type: ChatType,
    /// When the message was sent
    pub timestamp: SystemTime,
    /// Game/room context (if any)
    pub game_context: Option<String>,
    /// Whether this is a private message
    pub is_private: bool,
    /// Target player for private messages
    pub target_player: Option<IpAddr>,
}

impl ChatMessage {
    /// Create a new chat message
    pub fn new(
        sender_name: String,
        sender_ip: IpAddr,
        message: String,
        chat_type: ChatType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_name,
            sender_ip,
            message,
            chat_type,
            timestamp: SystemTime::now(),
            game_context: None,
            is_private: false,
            target_player: None,
        }
    }

    /// Create a system message
    pub fn system(message: String) -> Self {
        let system_ip: IpAddr = "127.0.0.1".parse().unwrap();
        Self {
            id: Uuid::new_v4(),
            sender_name: "System".to_string(),
            sender_ip: system_ip,
            message,
            chat_type: ChatType::System,
            timestamp: SystemTime::now(),
            game_context: None,
            is_private: false,
            target_player: None,
        }
    }

    /// Create a private message
    pub fn private(
        sender_name: String,
        sender_ip: IpAddr,
        target_ip: IpAddr,
        message: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_name,
            sender_ip,
            message,
            chat_type: ChatType::Normal,
            timestamp: SystemTime::now(),
            game_context: None,
            is_private: true,
            target_player: Some(target_ip),
        }
    }

    /// Get formatted display string
    pub fn get_display_string(&self) -> String {
        let time = self
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let time_str = format!("{:02}:{:02}", (time / 60) % 60, time % 60);

        match self.chat_type {
            ChatType::Normal => {
                if self.is_private {
                    format!(
                        "[{}] {} (private): {}",
                        time_str, self.sender_name, self.message
                    )
                } else {
                    format!("[{}] {}: {}", time_str, self.sender_name, self.message)
                }
            }
            ChatType::Emote => {
                format!("[{}] * {} {}", time_str, self.sender_name, self.message)
            }
            ChatType::System => {
                format!("[{}] *** {} ***", time_str, self.message)
            }
        }
    }

    /// Check if this message should be displayed to a specific player
    pub fn is_visible_to(&self, player_ip: IpAddr) -> bool {
        if self.is_private {
            // Private messages are only visible to sender and target
            self.sender_ip == player_ip || self.target_player == Some(player_ip)
        } else {
            // Public messages are visible to everyone
            true
        }
    }

    /// Get message priority for delivery
    pub fn get_priority(&self) -> u8 {
        match self.chat_type {
            ChatType::System => 0, // Highest priority
            ChatType::Normal => 1,
            ChatType::Emote => 2, // Lowest priority
        }
    }
}

/// Chat history management
#[derive(Debug)]
pub struct ChatHistory {
    messages: VecDeque<ChatMessage>,
    max_messages: usize,
}

impl ChatHistory {
    /// Create a new chat history
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_messages),
            max_messages,
        }
    }

    /// Add a message to history
    pub fn add_message(&mut self, message: ChatMessage) {
        if self.messages.len() >= self.max_messages {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    /// Get recent messages
    pub fn get_recent_messages(&self, count: usize) -> Vec<&ChatMessage> {
        self.messages.iter().rev().take(count).collect()
    }

    /// Get all messages visible to a player
    pub fn get_messages_for_player(&self, player_ip: IpAddr) -> Vec<&ChatMessage> {
        self.messages
            .iter()
            .filter(|msg| msg.is_visible_to(player_ip))
            .collect()
    }

    /// Get messages since a timestamp
    pub fn get_messages_since(&self, since: SystemTime) -> Vec<&ChatMessage> {
        self.messages
            .iter()
            .filter(|msg| msg.timestamp >= since)
            .collect()
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Chat statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatStats {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Messages by type
    pub messages_by_type: HashMap<String, u64>,
    /// Active participants
    pub active_participants: HashMap<IpAddr, String>,
    /// Last activity time
    pub last_activity: Option<SystemTime>,
}

impl ChatStats {
    /// Record a sent message
    pub fn record_sent(&mut self, chat_type: ChatType) {
        self.messages_sent += 1;
        let type_str = format!("{:?}", chat_type);
        *self.messages_by_type.entry(type_str).or_insert(0) += 1;
        self.last_activity = Some(SystemTime::now());
    }

    /// Record a received message
    pub fn record_received(&mut self, sender_ip: IpAddr, sender_name: String, chat_type: ChatType) {
        self.messages_received += 1;
        let type_str = format!("{:?}", chat_type);
        *self.messages_by_type.entry(type_str).or_insert(0) += 1;
        self.active_participants.insert(sender_ip, sender_name);
        self.last_activity = Some(SystemTime::now());
    }
}

/// LAN chat system
pub struct LanChat {
    /// Maximum message length
    max_message_length: usize,
    /// Base UDP port for LAN communication
    base_port: u16,
    /// Chat history
    history: Arc<RwLock<ChatHistory>>,
    /// UDP socket for chat communication
    socket: Arc<RwLock<Option<UdpSocket>>>,
    /// Encryption helper shared with lobby/discovery.
    crypto: LanCrypto,
    /// Known players for chat
    players: Arc<RwLock<HashMap<IpAddr, String>>>,
    /// Bridge into the parent [`LanApi`] background task.
    bridge_tx: LanEventSender,
    /// Background tasks
    tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    /// Whether chat is active
    is_active: Arc<RwLock<bool>>,
    /// Chat statistics
    stats: Arc<RwLock<ChatStats>>,
    /// Message rate limiting
    rate_limiter: Arc<RwLock<RateLimiter>>,
    /// Local player info
    local_player_name: Arc<RwLock<String>>,
    local_player_ip: Arc<RwLock<Option<IpAddr>>>,
    shutdown_notify: Arc<Notify>,
}

/// Simple rate limiter for chat messages
#[derive(Debug)]
struct RateLimiter {
    messages: VecDeque<NetworkInstant>,
    max_messages: usize,
    window: Duration,
}

impl RateLimiter {
    fn new(max_messages: usize, window: Duration) -> Self {
        Self {
            messages: VecDeque::new(),
            max_messages,
            window,
        }
    }

    fn can_send(&mut self) -> bool {
        let now = NetworkInstant::now();

        // Remove old messages outside the window
        while let Some(&front_time) = self.messages.front() {
            if front_time.elapsed() > self.window {
                self.messages.pop_front();
            } else {
                break;
            }
        }

        // Check if we can send
        if self.messages.len() < self.max_messages {
            self.messages.push_back(now);
            true
        } else {
            false
        }
    }
}

impl LanChat {
    /// Create a new LAN chat system
    pub async fn new(
        max_message_length: usize,
        base_port: u16,
        bridge_tx: LanEventSender,
    ) -> NetworkResult<Self> {
        Self::with_dependencies(max_message_length, base_port, bridge_tx, None, None).await
    }

    /// Create a chat subsystem with optional security context for encryption.
    pub async fn with_dependencies(
        max_message_length: usize,
        base_port: u16,
        bridge_tx: LanEventSender,
        security: Option<Arc<SecurityManager>>,
        connections: Option<Arc<RwLock<ConnectionManager>>>,
    ) -> NetworkResult<Self> {
        Ok(Self {
            max_message_length,
            base_port,
            history: Arc::new(RwLock::new(ChatHistory::new(1000))),
            socket: Arc::new(RwLock::new(None)),
            crypto: LanCrypto::new(security, connections),
            players: Arc::new(RwLock::new(HashMap::new())),
            bridge_tx,
            tasks: Arc::new(RwLock::new(Vec::new())),
            is_active: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(ChatStats::default())),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(10, Duration::from_secs(60)))),
            local_player_name: Arc::new(RwLock::new("Player".to_string())),
            local_player_ip: Arc::new(RwLock::new(None)),
            shutdown_notify: Arc::new(Notify::new()),
        })
    }

    /// Initialize the chat system
    pub async fn init(&mut self) -> NetworkResult<()> {
        info!("Initializing LAN chat system");

        *self.is_active.write().await = true;
        self.shutdown_notify = Arc::new(Notify::new());

        // Initialize UDP socket for chat
        // Note: In a real implementation, this might share the same socket as the main lobby
        // For now, we'll create a separate one
        self.init_socket().await?;

        // Start background tasks
        self.start_background_tasks().await;

        info!("LAN chat system initialized");
        Ok(())
    }

    /// Initialize UDP socket
    async fn init_socket(&self) -> NetworkResult<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| NetworkError::transport(format!("Failed to bind chat socket: {}", e)))?;

        *self.socket.write().await = Some(socket);
        debug!("Chat UDP socket initialized");
        Ok(())
    }

    /// Start background tasks
    async fn start_background_tasks(&self) {
        // Start message receiver
        self.start_message_receiver().await;

        // Start stats updater
        self.start_stats_updater().await;
    }

    /// Start message receiver task
    async fn start_message_receiver(&self) {
        let socket = Arc::clone(&self.socket);
        let bridge_tx = self.bridge_tx.clone();
        let history = Arc::clone(&self.history);
        let stats = Arc::clone(&self.stats);
        let is_active = Arc::clone(&self.is_active);
        let shutdown = Arc::clone(&self.shutdown_notify);
        let crypto = self.crypto.clone();

        let handle = tokio::spawn(async move {
            let mut buffer = [0u8; 2048];

            'recv_loop: loop {
                let recv_result;

                {
                    let guard = socket.read().await;
                    if let Some(ref sock) = *guard {
                        recv_result = tokio::select! {
                            _ = shutdown.notified() => break 'recv_loop,
                            res = sock.recv_from(&mut buffer) => res,
                        };
                    } else {
                        drop(guard);
                        if tokio::select! {
                            _ = shutdown.notified() => true,
                            _ = tokio::time::sleep(Duration::from_millis(100)) => false,
                        } {
                            break;
                        } else {
                            continue;
                        }
                    }
                }

                if !*is_active.read().await {
                    continue;
                }

                match recv_result {
                    Ok((len, sender)) => {
                        trace!("Received chat data from {}: {} bytes", sender, len);

                        if len == 0 {
                            continue;
                        }

                        match crypto.decode(&buffer[..len], sender).await {
                            Ok(plaintext) => {
                                match serde_json::from_slice::<ChatMessage>(&plaintext) {
                                    Ok(message) => {
                                        debug!(
                                            "Received chat message from {}: {}",
                                            message.sender_name, message.message
                                        );

                                        {
                                            let mut hist = history.write().await;
                                            hist.add_message(message.clone());
                                        }

                                        {
                                            let mut stats_guard = stats.write().await;
                                            stats_guard.record_received(
                                                message.sender_ip,
                                                message.sender_name.clone(),
                                                message.chat_type,
                                            );
                                        }
                                        let _ = bridge_tx.send(LanBridgeEvent::ChatEvent(message));
                                    }
                                    Err(err) => {
                                        warn!(
                                            "Failed to parse chat payload from {}: {}",
                                            sender, err
                                        );
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Failed to decrypt chat payload from {}: {}", sender, err);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Chat socket receive error: {}", e);
                    }
                }
            }

            debug!("Chat message receiver stopped");
        });

        self.tasks.write().await.push(handle);
    }

    /// Start stats updater task
    async fn start_stats_updater(&self) {
        let stats = Arc::clone(&self.stats);
        let is_active = Arc::clone(&self.is_active);
        let shutdown = Arc::clone(&self.shutdown_notify);

        let handle = tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = shutdown.notified() => {
                        break;
                    }
                    _ = tick.tick() => {
                        if !*is_active.read().await {
                            continue;
                        }

                        let mut stats_guard = stats.write().await;
                        if let Some(last_activity) = stats_guard.last_activity {
                            if SystemTime::now()
                                .duration_since(last_activity)
                                .unwrap_or_default()
                                > Duration::from_secs(300)
                            {
                                stats_guard.active_participants.clear();
                            }
                        }
                    }
                }
            }

            debug!("Chat stats updater stopped");
        });

        self.tasks.write().await.push(handle);
    }

    /// Set local player information
    pub async fn set_local_player(&self, name: String, ip: IpAddr) {
        *self.local_player_name.write().await = name;
        *self.local_player_ip.write().await = Some(ip);
    }

    /// Send a chat message
    pub async fn send_message(&self, message: String, chat_type: ChatType) -> NetworkResult<()> {
        // Validate message length
        if message.len() > self.max_message_length {
            return Err(NetworkError::invalid_command(format!(
                "Message too long: {} > {}",
                message.len(),
                self.max_message_length
            )));
        }

        // Check rate limiting
        {
            let mut limiter = self.rate_limiter.write().await;
            if !limiter.can_send() {
                return Err(NetworkError::invalid_command(
                    "Rate limit exceeded".to_string(),
                ));
            }
        }

        // Get local player info
        let sender_name = self.local_player_name.read().await.clone();
        let sender_ip = match *self.local_player_ip.read().await {
            Some(ip) => ip,
            None => {
                return Err(NetworkError::invalid_command(
                    "Local player IP not set".to_string(),
                ));
            }
        };

        // Create message
        let chat_message = ChatMessage::new(sender_name, sender_ip, message, chat_type);

        // Add to local history
        {
            let mut history = self.history.write().await;
            history.add_message(chat_message.clone());
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.record_sent(chat_type);
        }

        // Broadcast message
        self.broadcast_message(&chat_message).await?;

        debug!("Chat message sent: {}", chat_message.message);
        Ok(())
    }

    /// Send a private message
    pub async fn send_private_message(
        &self,
        target_ip: IpAddr,
        message: String,
    ) -> NetworkResult<()> {
        // Validate message length
        if message.len() > self.max_message_length {
            return Err(NetworkError::invalid_command(format!(
                "Message too long: {} > {}",
                message.len(),
                self.max_message_length
            )));
        }

        // Check rate limiting
        {
            let mut limiter = self.rate_limiter.write().await;
            if !limiter.can_send() {
                return Err(NetworkError::invalid_command(
                    "Rate limit exceeded".to_string(),
                ));
            }
        }

        // Get local player info
        let sender_name = self.local_player_name.read().await.clone();
        let sender_ip = match *self.local_player_ip.read().await {
            Some(ip) => ip,
            None => {
                return Err(NetworkError::invalid_command(
                    "Local player IP not set".to_string(),
                ));
            }
        };

        // Create private message
        let chat_message = ChatMessage::private(sender_name, sender_ip, target_ip, message);

        // Add to local history
        {
            let mut history = self.history.write().await;
            history.add_message(chat_message.clone());
        }

        // Send to specific target
        self.send_message_to(&chat_message, target_ip).await?;

        debug!("Private message sent to {}", target_ip);
        Ok(())
    }

    /// Broadcast a message to all players
    async fn broadcast_message(&self, message: &ChatMessage) -> NetworkResult<()> {
        let players = self.players.read().await;

        for &player_ip in players.keys() {
            if player_ip != message.sender_ip {
                // Don't send to sender
                if let Err(e) = self.send_message_to(message, player_ip).await {
                    warn!("Failed to send message to {}: {}", player_ip, e);
                }
            }
        }

        Ok(())
    }

    /// Send a message to a specific IP
    async fn send_message_to(&self, message: &ChatMessage, target_ip: IpAddr) -> NetworkResult<()> {
        if let Some(ref socket) = *self.socket.read().await {
            let target_addr = SocketAddr::new(target_ip, self.base_port);
            let message_data = serde_json::to_string(message)
                .map_err(|e| NetworkError::serialization(e.to_string()))?;

            let payload = self
                .crypto
                .encode(message_data.as_bytes(), target_addr)
                .await;
            socket.send_to(&payload, target_addr).await.map_err(|e| {
                NetworkError::transport(format!("Failed to send chat message: {}", e))
            })?;
        }

        Ok(())
    }

    /// Add a player to the chat system
    pub async fn add_player(&self, ip: IpAddr, name: String) {
        let mut players = self.players.write().await;
        players.insert(ip, name.clone());

        // Send system message about player joining
        let system_msg = ChatMessage::system(format!("{} joined the chat", name));
        let mut history = self.history.write().await;
        history.add_message(system_msg);

        debug!("Added player to chat: {} ({})", name, ip);
    }

    /// Remove a player from the chat system
    pub async fn remove_player(&self, ip: IpAddr) -> Option<String> {
        let mut players = self.players.write().await;
        let player_name = players.remove(&ip);

        if let Some(ref name) = player_name {
            // Send system message about player leaving
            let system_msg = ChatMessage::system(format!("{} left the chat", name));
            let mut history = self.history.write().await;
            history.add_message(system_msg);

            debug!("Removed player from chat: {} ({})", name, ip);
        }

        player_name
    }

    /// Get recent chat messages
    pub async fn get_recent_messages(&self, count: usize) -> Vec<ChatMessage> {
        let history = self.history.read().await;
        history
            .get_recent_messages(count)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get messages for a specific player
    pub async fn get_messages_for_player(&self, player_ip: IpAddr) -> Vec<ChatMessage> {
        let history = self.history.read().await;
        history
            .get_messages_for_player(player_ip)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Clear chat history
    pub async fn clear_history(&self) {
        let mut history = self.history.write().await;
        history.clear();

        info!("Chat history cleared");
    }

    /// Get chat statistics
    pub async fn get_stats(&self) -> ChatStats {
        self.stats.read().await.clone()
    }

    /// Update chat system
    pub async fn update(&mut self) -> NetworkResult<()> {
        // Nothing specific to update for now
        // Message receiving is handled by background tasks
        Ok(())
    }

    /// Shutdown the chat system
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down LAN chat system");

        *self.is_active.write().await = false;
        self.shutdown_notify.notify_waiters();

        let mut tasks = self.tasks.write().await;
        for handle in tasks.drain(..) {
            handle.abort();
            let _ = handle.await;
        }

        *self.socket.write().await = None;

        {
            let mut players = self.players.write().await;
            players.clear();
        }

        info!("LAN chat system shut down successfully");
        self.shutdown_notify = Arc::new(Notify::new());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan_api::lan_event_channel;
    use std::net::Ipv4Addr;

    #[test]
    fn test_chat_message_creation() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let message = ChatMessage::new(
            "TestPlayer".to_string(),
            ip,
            "Hello world!".to_string(),
            ChatType::Normal,
        );

        assert_eq!(message.sender_name, "TestPlayer");
        assert_eq!(message.sender_ip, ip);
        assert_eq!(message.message, "Hello world!");
        assert_eq!(message.chat_type, ChatType::Normal);
        assert!(!message.is_private);
    }

    #[test]
    fn test_system_message() {
        let message = ChatMessage::system("Game started".to_string());

        assert_eq!(message.sender_name, "System");
        assert_eq!(message.message, "Game started");
        assert_eq!(message.chat_type, ChatType::System);
        assert!(!message.is_private);
    }

    #[test]
    fn test_private_message() {
        let sender_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let target_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));

        let message = ChatMessage::private(
            "Sender".to_string(),
            sender_ip,
            target_ip,
            "Secret message".to_string(),
        );

        assert_eq!(message.sender_name, "Sender");
        assert_eq!(message.message, "Secret message");
        assert!(message.is_private);
        assert_eq!(message.target_player, Some(target_ip));

        // Test visibility
        assert!(message.is_visible_to(sender_ip));
        assert!(message.is_visible_to(target_ip));
        assert!(!message.is_visible_to(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 3))));
    }

    #[test]
    fn test_message_display_formatting() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

        // Normal message
        let normal = ChatMessage::new(
            "Player".to_string(),
            ip,
            "Hello".to_string(),
            ChatType::Normal,
        );
        let display = normal.get_display_string();
        assert!(display.contains("Player: Hello"));

        // Emote message
        let emote = ChatMessage::new(
            "Player".to_string(),
            ip,
            "waves".to_string(),
            ChatType::Emote,
        );
        let display = emote.get_display_string();
        assert!(display.contains("* Player waves"));

        // System message
        let system = ChatMessage::system("Important".to_string());
        let display = system.get_display_string();
        assert!(display.contains("*** Important ***"));
    }

    #[test]
    fn test_chat_history() {
        let mut history = ChatHistory::new(3);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

        // Add messages
        for i in 1..=5 {
            let msg = ChatMessage::new(
                "Player".to_string(),
                ip,
                format!("Message {}", i),
                ChatType::Normal,
            );
            history.add_message(msg);
        }

        // Should only keep last 3 messages
        assert_eq!(history.len(), 3);

        let recent = history.get_recent_messages(2);
        assert_eq!(recent.len(), 2);
        assert!(recent[0].message.contains("Message 5")); // Most recent first
        assert!(recent[1].message.contains("Message 4"));
    }

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(1));

        // Should allow first 3 messages
        assert!(limiter.can_send());
        assert!(limiter.can_send());
        assert!(limiter.can_send());

        // Should deny 4th message
        assert!(!limiter.can_send());

        // Wait and should allow again (in real test, would need to wait)
        // For test, we'll just verify the structure is correct
        assert_eq!(limiter.messages.len(), 3);
    }

    #[tokio::test]
    async fn test_chat_system_creation() {
        let (tx, _rx) = lan_event_channel();
        let chat = LanChat::new(100, 8086, tx).await.unwrap();

        assert_eq!(chat.max_message_length, 100);
        assert!(!*chat.is_active.read().await);

        let history = chat.history.read().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_player_management() {
        let (tx, _rx) = lan_event_channel();
        let chat = LanChat::new(100, 8086, tx).await.unwrap();

        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        // Add player
        chat.add_player(ip, "TestPlayer".to_string()).await;

        let players = chat.players.read().await;
        assert_eq!(players.get(&ip), Some(&"TestPlayer".to_string()));

        // Check system message was added
        let history = chat.history.read().await;
        assert_eq!(history.len(), 1);
        assert!(history.get_recent_messages(1)[0].message.contains("joined"));
    }
}
