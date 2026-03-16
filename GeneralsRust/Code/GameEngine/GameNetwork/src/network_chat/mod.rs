//! Network-Integrated Chat System
//!
//! This module provides a complete chat system that integrates with both
//! the game UI and network transport layers for multiplayer chat functionality.

pub mod chat_protocol;
pub mod chat_router;
pub mod chat_filter;
pub mod chat_history;
pub mod emoticons;
pub mod typing_indicator;
pub mod chat_moderation;

pub use chat_protocol::*;
pub use chat_router::*;
pub use chat_filter::*;
pub use chat_history::*;
pub use emoticons::*;
pub use typing_indicator::*;
pub use chat_moderation::*;

use crate::error::{NetworkError, NetworkResult};
use crate::gamespy::{ChatMessage, ChatMessageType, GameSpyEvent};
use crate::lan_api::chat::{ChatMessage as LanChatMessage, ChatType as LanChatType};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

/// Unified chat message that works across all network backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatMessage {
    /// Unique message ID
    pub id: String,
    /// Sender player ID
    pub sender_id: u32,
    /// Sender display name
    pub sender_name: String,
    /// Message content
    pub message: String,
    /// Chat channel/type
    pub channel: ChatChannel,
    /// Message type
    pub message_type: ChatMessageType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Target player (for private messages)
    pub target_player: Option<u32>,
    /// Emoticon data (if applicable)
    pub emoticon: Option<EmoticonData>,
    /// Whether message was filtered
    pub was_filtered: bool,
    /// Original unfiltered message
    pub original_message: Option<String>,
}

/// Chat channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChatChannel {
    /// Global chat - all players
    Global,
    /// Team/allies chat
    Allies,
    /// Private message
    Private(u32),
    /// System notification
    System,
    /// Observer chat
    Observers,
}

/// Emoticon data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmoticonData {
    /// Emoticon name/ID
    pub name: String,
    /// Emoticon shortcut (e.g., ":)")
    pub shortcut: String,
    /// Custom image data (if applicable)
    pub image_data: Option<Vec<u8>>,
}

/// Player typing status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingStatus {
    /// Player ID
    pub player_id: u32,
    /// Player name
    pub player_name: String,
    /// Is currently typing
    pub is_typing: bool,
    /// Last update timestamp
    pub timestamp: DateTime<Utc>,
}

/// Chat moderation action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationAction {
    /// Action type
    pub action: ModerationActionType,
    /// Target player ID
    pub target_player: u32,
    /// Moderator player ID
    pub moderator: u32,
    /// Reason
    pub reason: String,
    /// Duration (for temporary actions)
    pub duration_seconds: Option<u64>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationActionType {
    Mute,
    Unmute,
    Kick,
    Ban,
    Warning,
}

/// Network-integrated chat system
pub struct NetworkChatSystem {
    /// Local player ID
    local_player_id: Arc<RwLock<u32>>,
    /// Local player name
    local_player_name: Arc<RwLock<String>>,
    /// Chat router for message routing
    router: Arc<ChatRouter>,
    /// Message filter
    filter: Arc<ChatFilter>,
    /// Chat history manager
    history: Arc<ChatHistoryManager>,
    /// Typing indicator tracker
    typing: Arc<TypingIndicator>,
    /// Moderation system
    moderation: Arc<ChatModeration>,
    /// Event sender
    event_tx: broadcast::Sender<ChatEvent>,
    /// GameSpy event receiver
    gamespy_rx: mpsc::UnboundedReceiver<GameSpyEvent>,
    /// LAN chat event receiver
    lan_rx: mpsc::UnboundedReceiver<LanChatMessage>,
    /// Active chat channels
    active_channels: Arc<RwLock<HashSet<ChatChannel>>>,
    /// Muted players
    muted_players: Arc<RwLock<HashSet<u32>>>,
    /// Blocked players
    blocked_players: Arc<RwLock<HashSet<u32>>>,
}

/// Chat events
#[derive(Debug, Clone)]
pub enum ChatEvent {
    Message(UnifiedChatMessage),
    Typing(TypingStatus),
    Moderation(ModerationAction),
    System(String),
    PlayerJoined(u32, String),
    PlayerLeft(u32),
    ChannelChanged(ChatChannel),
}

impl NetworkChatSystem {
    /// Create new network chat system
    pub async fn new(
        local_player_id: u32,
        local_player_name: String,
        event_tx: broadcast::Sender<ChatEvent>,
    ) -> NetworkResult<Self> {
        let (gamespy_tx, gamespy_rx) = mpsc::unbounded_channel();
        let (lan_tx, lan_rx) = mpsc::unbounded_channel();

        Ok(Self {
            local_player_id: Arc::new(RwLock::new(local_player_id)),
            local_player_name: Arc::new(RwLock::new(local_player_name)),
            router: Arc::new(ChatRouter::new()),
            filter: Arc::new(ChatFilter::new()),
            history: Arc::new(ChatHistoryManager::new(1000)),
            typing: Arc::new(TypingIndicator::new()),
            moderation: Arc::new(ChatModeration::new()),
            event_tx,
            gamespy_rx,
            lan_rx,
            active_channels: Arc::new(RwLock::new(HashSet::new())),
            muted_players: Arc::new(RwLock::new(HashSet::new())),
            blocked_players: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Initialize the chat system
    pub async fn initialize(&mut self) -> NetworkResult<()> {
        info!("Initializing network chat system");

        // Add default channels
        {
            let mut channels = self.active_channels.write().await;
            channels.insert(ChatChannel::Global);
            channels.insert(ChatChannel::Allies);
            channels.insert(ChatChannel::Observers);
        }

        // Start event processing
        self.start_event_processing().await;

        info!("Network chat system initialized");
        Ok(())
    }

    /// Send a chat message
    pub async fn send_message(
        &self,
        message: String,
        channel: ChatChannel,
    ) -> NetworkResult<()> {
        // Check if player is muted
        let local_id = *self.local_player_id.read().await;
        if self.moderation.is_player_muted(local_id).await {
            return Err(NetworkError::invalid_command("You are muted".to_string()));
        }

        // Filter the message
        let (filtered_message, was_filtered) = self.filter.filter_message(&message);

        // Create unified message
        let sender_name = self.local_player_name.read().await.clone();
        let chat_message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: local_id,
            sender_name,
            message: filtered_message.clone(),
            channel,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: match channel {
                ChatChannel::Private(id) => Some(id),
                _ => None,
            },
            emoticon: None,
            was_filtered,
            original_message: if was_filtered { Some(message) } else { None },
        };

        // Add to history
        self.history.add_message(chat_message.clone()).await;

        // Route message
        self.router.route_message(chat_message.clone()).await?;

        // Emit event
        let _ = self.event_tx.send(ChatEvent::Message(chat_message));

        debug!("Chat message sent: {}", filtered_message);
        Ok(())
    }

    /// Send an emote
    pub async fn send_emote(
        &self,
        emote: String,
        channel: ChatChannel,
    ) -> NetworkResult<()> {
        let local_id = *self.local_player_id.read().await;
        let sender_name = self.local_player_name.read().await.clone();

        let chat_message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: local_id,
            sender_name,
            message: emote.clone(),
            channel,
            message_type: ChatMessageType::Emote,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: Some(EmoticonData {
                name: emote.clone(),
                shortcut: emote,
                image_data: None,
            }),
            was_filtered: false,
            original_message: None,
        };

        self.history.add_message(chat_message.clone()).await;
        self.router.route_message(chat_message.clone()).await?;

        let _ = self.event_tx.send(ChatEvent::Message(chat_message));
        Ok(())
    }

    /// Send private message
    pub async fn send_private_message(
        &self,
        target_player: u32,
        message: String,
    ) -> NetworkResult<()> {
        self.send_message(message, ChatChannel::Private(target_player)).await
    }

    /// Set typing status
    pub async fn set_typing(&self, is_typing: bool) {
        let local_id = *self.local_player_id.read().await;
        let local_name = self.local_player_name.read().await.clone();

        let status = TypingStatus {
            player_id: local_id,
            player_name: local_name,
            is_typing,
            timestamp: Utc::now(),
        };

        self.typing.update_status(status.clone()).await;
        let _ = self.event_tx.send(ChatEvent::Typing(status));
    }

    /// Mute a player
    pub async fn mute_player(&self, player_id: u32, duration_seconds: Option<u64>) {
        self.muted_players.write().await.insert(player_id);
        self.moderation.mute_player(player_id, duration_seconds).await;

        let action = ModerationAction {
            action: ModerationActionType::Mute,
            target_player: player_id,
            moderator: *self.local_player_id.read().await,
            reason: "Muted by player".to_string(),
            duration_seconds,
            timestamp: Utc::now(),
        };
        let _ = self.event_tx.send(ChatEvent::Moderation(action));
    }

    /// Block a player
    pub async fn block_player(&self, player_id: u32) {
        self.blocked_players.write().await.insert(player_id);
        info!("Blocked player: {}", player_id);
    }

    /// Get chat history
    pub async fn get_history(&self, count: usize) -> Vec<UnifiedChatMessage> {
        self.history.get_recent(count).await
    }

    /// Get typing indicators
    pub async fn get_typing_players(&self) -> Vec<TypingStatus> {
        self.typing.get_active_typers().await
    }

    /// Check if player is muted
    pub async fn is_player_muted(&self, player_id: u32) -> bool {
        self.muted_players.read().await.contains(&player_id)
    }

    /// Check if player is blocked
    pub async fn is_player_blocked(&self, player_id: u32) -> bool {
        self.blocked_players.read().await.contains(&player_id)
    }

    /// Process incoming GameSpy chat event
    async fn process_gamespy_event(&mut self, event: GameSpyEvent) -> NetworkResult<()> {
        match event {
            GameSpyEvent::ChatMessage(msg) => {
                // Convert to unified message
                let unified = Self::convert_gamespy_message(msg).await?;
                self.handle_incoming_message(unified).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Process incoming LAN chat message
    async fn process_lan_message(&mut self, msg: LanChatMessage) -> NetworkResult<()> {
        let unified = Self::convert_lan_message(msg).await?;
        self.handle_incoming_message(unified).await
    }

    /// Handle incoming unified message
    async fn handle_incoming_message(&self, msg: UnifiedChatMessage) -> NetworkResult<()> {
        // Check if sender is blocked
        if self.is_player_blocked(msg.sender_id).await {
            debug!("Dropping message from blocked player: {}", msg.sender_id);
            return Ok(());
        }

        // Check if sender is muted
        if self.is_player_muted(msg.sender_id).await {
            debug!("Dropping message from muted player: {}", msg.sender_id);
            return Ok(());
        }

        // Add to history
        self.history.add_message(msg.clone()).await;

        // Emit event
        let _ = self.event_tx.send(ChatEvent::Message(msg));

        Ok(())
    }

    /// Convert GameSpy message to unified
    async fn convert_gamespy_message(msg: ChatMessage) -> NetworkResult<UnifiedChatMessage> {
        // Parse sender ID from GameSpy format
        let sender_id = msg.sender.parse::<u32>().unwrap_or(0);

        Ok(UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id,
            sender_name: msg.sender.clone(),
            message: msg.message.clone(),
            channel: if msg.message_type == ChatMessageType::Private {
                ChatChannel::Private(sender_id)
            } else {
                ChatChannel::Global
            },
            message_type: msg.message_type,
            timestamp: msg.timestamp,
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        })
    }

    /// Convert LAN message to unified
    async fn convert_lan_message(msg: LanChatMessage) -> NetworkResult<UnifiedChatMessage> {
        use std::net::IpAddr;

        // Generate player ID from IP
        let sender_id = match msg.sender_ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                ((octets[0] as u32) << 24) |
                ((octets[1] as u32) << 16) |
                ((octets[2] as u32) << 8) |
                (octets[3] as u32)
            }
            IpAddr::V6(_ipv6) => 0, // Simplified
        };

        let (message_type, channel) = match msg.chat_type {
            LanChatType::Normal => (ChatMessageType::Normal, ChatChannel::Global),
            LanChatType::Emote => (ChatMessageType::Emote, ChatChannel::Global),
            LanChatType::System => (ChatMessageType::System, ChatChannel::System),
        };

        Ok(UnifiedChatMessage {
            id: msg.id.to_string(),
            sender_id,
            sender_name: msg.sender_name.clone(),
            message: msg.message.clone(),
            channel,
            message_type,
            timestamp: Utc::now(),
            target_player: msg.target_player.map(|ip| 0), // Simplified
            emoticon: None,
            was_filtered: false,
            original_message: None,
        })
    }

    /// Start event processing loop
    async fn start_event_processing(&mut self) {
        // Process GameSpy events
        // Process LAN events
        // This would be implemented with proper async task spawning
    }

    /// Shutdown chat system
    pub async fn shutdown(&self) -> NetworkResult<()> {
        info!("Shutting down network chat system");

        // Save chat history
        self.history.save_to_disk().await
            .map_err(|e| NetworkError::Connection { message: format!("Failed to save chat history: {}", e) })?;

        info!("Network chat system shut down");
        Ok(())
    }
}

/// Chat protocol version
pub const CHAT_PROTOCOL_VERSION: u32 = 1;

/// Maximum message length
pub const MAX_MESSAGE_LENGTH: usize = 512;

/// Maximum chat history size
pub const MAX_CHAT_HISTORY: usize = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_message_creation() {
        let message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 1,
            sender_name: "TestPlayer".to_string(),
            message: "Hello, world!".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        assert_eq!(message.sender_id, 1);
        assert_eq!(message.message, "Hello, world!");
        assert_eq!(message.channel, ChatChannel::Global);
    }

    #[test]
    fn test_chat_channel_equality() {
        assert_eq!(ChatChannel::Global, ChatChannel::Global);
        assert_ne!(ChatChannel::Global, ChatChannel::Allies);
        assert_eq!(ChatChannel::Private(1), ChatChannel::Private(1));
        assert_ne!(ChatChannel::Private(1), ChatChannel::Private(2));
    }
}
