//! Chat Router Module
//!
//! Routes chat messages to appropriate recipients based on channel and player relationships

use crate::error::{NetworkError, NetworkResult};
use crate::network_chat::{ChatChannel, UnifiedChatMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, trace};

/// Player relationship information
#[derive(Debug, Clone, Copy)]
pub struct PlayerRelationship {
    /// Player ID
    pub player_id: u32,
    /// Team ID (0 = no team)
    pub team_id: u32,
    /// Is ally
    pub is_ally: bool,
    /// Is observer
    pub is_observer: bool,
}

/// Chat router for message distribution
pub struct ChatRouter {
    /// Player relationships
    relationships: Arc<RwLock<HashMap<u32, PlayerRelationship>>>,
    /// Channel subscribers
    channel_subscribers: Arc<RwLock<HashMap<ChatChannel, Vec<u32>>>>,
    /// Message sender for each player
    player_senders: Arc<RwLock<HashMap<u32, mpsc::UnboundedSender<UnifiedChatMessage>>>>,
}

impl ChatRouter {
    /// Create new chat router
    pub fn new() -> Self {
        Self {
            relationships: Arc::new(RwLock::new(HashMap::new())),
            channel_subscribers: Arc::new(RwLock::new(HashMap::new())),
            player_senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update player relationship
    pub async fn update_relationship(&self, relationship: PlayerRelationship) {
        let mut relationships = self.relationships.write().await;
        relationships.insert(relationship.player_id, relationship);
        trace!("Updated relationship for player {}", relationship.player_id);
    }

    /// Remove player
    pub async fn remove_player(&self, player_id: u32) {
        let mut relationships = self.relationships.write().await;
        relationships.remove(&player_id);

        let mut senders = self.player_senders.write().await;
        senders.remove(&player_id);

        // Remove from channel subscriptions
        let mut subscribers = self.channel_subscribers.write().await;
        for subscribers_list in subscribers.values_mut() {
            subscribers_list.retain(|&id| id != player_id);
        }

        debug!("Removed player {} from chat router", player_id);
    }

    /// Subscribe player to channel
    pub async fn subscribe_channel(&self, player_id: u32, channel: ChatChannel) {
        let mut subscribers = self.channel_subscribers.write().await;
        subscribers.entry(channel).or_insert_with(Vec::new).push(player_id);
        debug!("Player {} subscribed to channel {:?}", player_id, channel);
    }

    /// Unsubscribe player from channel
    pub async fn unsubscribe_channel(&self, player_id: u32, channel: &ChatChannel) {
        let mut subscribers = self.channel_subscribers.write().await;
        if let Some(subscribers_list) = subscribers.get_mut(channel) {
            subscribers_list.retain(|id: &u32| *id != player_id);
        }
        debug!("Player {} unsubscribed from channel {:?}", player_id, channel);
    }

    /// Register player message sender
    pub async fn register_sender(&self, player_id: u32, sender: mpsc::UnboundedSender<UnifiedChatMessage>) {
        let mut senders = self.player_senders.write().await;
        senders.insert(player_id, sender);
        debug!("Registered message sender for player {}", player_id);
    }

    /// Route message to appropriate recipients
    pub async fn route_message(&self, message: UnifiedChatMessage) -> NetworkResult<()> {
        let recipients = self.calculate_recipients(&message).await?;

        debug!("Routing message to {} recipients", recipients.len());

        for player_id in recipients {
            self.send_to_player(player_id, message.clone()).await?;
        }

        Ok(())
    }

    /// Calculate which players should receive a message
    async fn calculate_recipients(&self, message: &UnifiedChatMessage) -> NetworkResult<Vec<u32>> {
        let relationships = self.relationships.read().await;
        let mut recipients = Vec::new();

        match message.channel {
            ChatChannel::Global => {
                // Send to all non-observer players
                for (&player_id, &rel) in relationships.iter() {
                    if player_id != message.sender_id && !rel.is_observer {
                        recipients.push(player_id);
                    }
                }
            }
            ChatChannel::Allies => {
                // Send to allies only
                let sender_team = relationships.get(&message.sender_id)
                    .map(|r| r.team_id)
                    .unwrap_or(0);

                for (&player_id, &rel) in relationships.iter() {
                    if player_id != message.sender_id && rel.team_id == sender_team {
                        recipients.push(player_id);
                    }
                }
            }
            ChatChannel::Private(target_id) => {
                // Send only to target
                recipients.push(target_id);
            }
            ChatChannel::System => {
                // System messages go to everyone
                for (&player_id, _) in relationships.iter() {
                    if player_id != message.sender_id {
                        recipients.push(player_id);
                    }
                }
            }
            ChatChannel::Observers => {
                // Send only to observers
                for (&player_id, &rel) in relationships.iter() {
                    if player_id != message.sender_id && rel.is_observer {
                        recipients.push(player_id);
                    }
                }
            }
        }

        Ok(recipients)
    }

    /// Send message to specific player
    async fn send_to_player(&self, player_id: u32, message: UnifiedChatMessage) -> NetworkResult<()> {
        let senders = self.player_senders.read().await;

        if let Some(sender) = senders.get(&player_id) {
            sender.send(message)
                .map_err(|e| NetworkError::transport(format!("Failed to send to player {}: {}", player_id, e)))?;
        } else {
            trace!("No sender registered for player {}", player_id);
        }

        Ok(())
    }

    /// Get players in channel
    pub async fn get_channel_players(&self, channel: ChatChannel) -> Vec<u32> {
        let subscribers = self.channel_subscribers.read().await;
        subscribers.get(&channel).cloned().unwrap_or_default()
    }

    /// Get player relationships
    pub async fn get_relationships(&self) -> HashMap<u32, PlayerRelationship> {
        self.relationships.read().await.clone()
    }

    /// Clear all routing data
    pub async fn clear(&self) {
        self.relationships.write().await.clear();
        self.channel_subscribers.write().await.clear();
        self.player_senders.write().await.clear();
        debug!("Cleared chat router");
    }
}

impl Default for ChatRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_creation() {
        let router = ChatRouter::new();
        let relationships = router.get_relationships().await;
        assert!(relationships.is_empty());
    }

    #[tokio::test]
    async fn test_relationship_update() {
        let router = ChatRouter::new();

        let rel = PlayerRelationship {
            player_id: 1,
            team_id: 0,
            is_ally: false,
            is_observer: false,
        };

        router.update_relationship(rel).await;

        let relationships = router.get_relationships().await;
        assert_eq!(relationships.len(), 1);
        assert_eq!(relationships[&1].player_id, 1);
    }

    #[tokio::test]
    async fn test_channel_subscription() {
        let router = ChatRouter::new();

        router.subscribe_channel(1, ChatChannel::Global).await;
        router.subscribe_channel(1, ChatChannel::Allies).await;

        let global_players = router.get_channel_players(ChatChannel::Global).await;
        assert_eq!(global_players.len(), 1);
        assert_eq!(global_players[0], 1);

        let allies_players = router.get_channel_players(ChatChannel::Allies).await;
        assert_eq!(allies_players.len(), 1);
    }

    #[tokio::test]
    async fn test_player_removal() {
        let router = ChatRouter::new();

        router.subscribe_channel(1, ChatChannel::Global).await;
        router.remove_player(1).await;

        let global_players = router.get_channel_players(ChatChannel::Global).await;
        assert!(global_players.is_empty());
    }

    #[tokio::test]
    async fn test_global_routing() {
        let router = ChatRouter::new();

        // Add players
        for i in 1..=3 {
            router.update_relationship(PlayerRelationship {
                player_id: i,
                team_id: 0,
                is_ally: false,
                is_observer: false,
            }).await;

            let (tx, _rx) = mpsc::unbounded_channel();
            router.register_sender(i, tx).await;
        }

        // Create global message
        let message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 1,
            sender_name: "Player1".to_string(),
            message: "Hello everyone".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: chrono::Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        // Route message (should go to players 2 and 3)
        let result = router.route_message(message).await;
        assert!(result.is_ok());
    }
}
