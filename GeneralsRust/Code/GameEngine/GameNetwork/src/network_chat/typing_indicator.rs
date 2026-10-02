//! Typing Indicator Module
//!
//! Tracks and broadcasts player typing status

use crate::network_chat::TypingStatus;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use tracing::trace;

/// Typing indicator tracker
///
/// Owned outright by the chat system; every field is touched only from the
/// owning struct's methods, so no lock is needed.
pub struct TypingIndicator {
    /// Player typing status
    typing_status: HashMap<u32, PlayerTypingState>,
    /// Typing timeout in seconds
    timeout_seconds: i64,
}

/// Player typing state
#[derive(Debug, Clone)]
struct PlayerTypingState {
    /// Player name
    player_name: String,
    /// Current typing status
    is_typing: bool,
    /// Last update timestamp
    last_update: DateTime<Utc>,
}

impl TypingIndicator {
    /// Create new typing indicator
    pub fn new() -> Self {
        Self {
            typing_status: HashMap::new(),
            timeout_seconds: 10,
        }
    }

    /// Update player typing status
    pub async fn update_status(&mut self, status: TypingStatus) {
        let state = PlayerTypingState {
            player_name: status.player_name.clone(),
            is_typing: status.is_typing,
            last_update: status.timestamp,
        };

        self.typing_status.insert(status.player_id, state);

        trace!(
            "Updated typing status for player {}: {}",
            status.player_id,
            status.is_typing
        );
    }

    /// Set player as typing
    pub async fn set_typing(&mut self, player_id: u32, player_name: String) {
        let status = TypingStatus {
            player_id,
            player_name,
            is_typing: true,
            timestamp: Utc::now(),
        };

        self.update_status(status).await;
    }

    /// Clear player typing status
    pub async fn clear_typing(&mut self, player_id: u32) {
        if let Some(state) = self.typing_status.get_mut(&player_id) {
            state.is_typing = false;
            state.last_update = Utc::now();
        }
    }

    /// Remove player
    pub async fn remove_player(&mut self, player_id: u32) {
        self.typing_status.remove(&player_id);
    }

    /// Get all currently typing players
    pub async fn get_active_typers(&self) -> Vec<TypingStatus> {
        let mut result = Vec::new();

        let now = Utc::now();
        let timeout = Duration::seconds(self.timeout_seconds);

        for (&player_id, state) in self.typing_status.iter() {
            // Check if status is recent enough
            if now.signed_duration_since(state.last_update).num_seconds() < self.timeout_seconds {
                if state.is_typing {
                    result.push(TypingStatus {
                        player_id,
                        player_name: state.player_name.clone(),
                        is_typing: true,
                        timestamp: state.last_update,
                    });
                }
            }
        }

        result
    }

    /// Get typing status for specific player
    pub async fn get_player_status(&self, player_id: u32) -> Option<TypingStatus> {
        self.typing_status.get(&player_id).map(|state| {
            // Check if status is still valid
            let now = Utc::now();
            let age = now.signed_duration_since(state.last_update).num_seconds();

            TypingStatus {
                player_id,
                player_name: state.player_name.clone(),
                is_typing: state.is_typing && age < self.timeout_seconds,
                timestamp: state.last_update,
            }
        })
    }

    /// Check if specific player is typing
    pub async fn is_player_typing(&self, player_id: u32) -> bool {
        if let Some(status) = self.get_player_status(player_id).await {
            status.is_typing
        } else {
            false
        }
    }

    /// Clean up expired typing indicators
    pub async fn cleanup_expired(&mut self) {
        let now = Utc::now();

        self.typing_status.retain(|_, state| {
            let age = now.signed_duration_since(state.last_update).num_seconds();
            age < self.timeout_seconds * 2 // Keep for twice the timeout period
        });
    }

    /// Clear all typing indicators
    pub async fn clear_all(&mut self) {
        self.typing_status.clear();
    }

    /// Get count of typing players
    pub async fn get_typing_count(&self) -> usize {
        self.get_active_typers().await.len()
    }
}

impl Default for TypingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typing_indicator_creation() {
        let mut indicator = TypingIndicator::new();
        let count = indicator.get_typing_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_set_typing() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "TestPlayer".to_string()).await;

        assert!(indicator.is_player_typing(1).await);
        assert_eq!(indicator.get_typing_count().await, 1);
    }

    #[tokio::test]
    async fn test_clear_typing() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "TestPlayer".to_string()).await;
        indicator.clear_typing(1).await;

        assert!(!indicator.is_player_typing(1).await);
    }

    #[tokio::test]
    async fn test_remove_player() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "TestPlayer".to_string()).await;
        indicator.remove_player(1).await;

        assert!(!indicator.is_player_typing(1).await);
        assert_eq!(indicator.get_typing_count().await, 0);
    }

    #[tokio::test]
    async fn test_multiple_players() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "Player1".to_string()).await;
        indicator.set_typing(2, "Player2".to_string()).await;
        indicator.set_typing(3, "Player3".to_string()).await;

        let typers = indicator.get_active_typers().await;
        assert_eq!(typers.len(), 3);
    }

    #[tokio::test]
    async fn test_get_player_status() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "TestPlayer".to_string()).await;

        let status = indicator.get_player_status(1).await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().player_name, "TestPlayer");

        let status = indicator.get_player_status(999).await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_clear_all() {
        let mut indicator = TypingIndicator::new();

        indicator.set_typing(1, "Player1".to_string()).await;
        indicator.set_typing(2, "Player2".to_string()).await;

        indicator.clear_all().await;

        assert_eq!(indicator.get_typing_count().await, 0);
    }
}
