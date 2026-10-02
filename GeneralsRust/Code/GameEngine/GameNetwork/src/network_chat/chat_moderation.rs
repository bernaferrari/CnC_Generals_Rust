//! Chat Moderation Module
//!
//! Provides moderation tools for managing chat behavior

use crate::network_chat::{ModerationAction, ModerationActionType};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Chat moderation system
///
/// Single-owner state: only this struct's methods touch the tables (mutators
/// take `&mut self`), so they are plain fields.
pub struct ChatModeration {
    /// Muted players
    muted_players: HashMap<u32, MuteRecord>,
    /// Banned players
    banned_players: HashMap<u32, BanRecord>,
    /// Warning history
    warnings: HashMap<u32, Vec<WarningRecord>>,
    /// Moderation log
    moderation_log: Vec<ModerationAction>,
}

/// Mute record
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MuteRecord {
    /// Player ID
    player_id: u32,
    /// Moderator who issued mute
    moderator_id: u32,
    /// Reason
    reason: String,
    /// When mute was issued
    issued_at: DateTime<Utc>,
    /// When mute expires (None = permanent)
    expires_at: Option<DateTime<Utc>>,
}

/// Ban record
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BanRecord {
    /// Player ID
    player_id: u32,
    /// Moderator who issued ban
    moderator_id: u32,
    /// Reason
    reason: String,
    /// When ban was issued
    issued_at: DateTime<Utc>,
    /// When ban expires (None = permanent)
    expires_at: Option<DateTime<Utc>>,
}

/// Warning record
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WarningRecord {
    /// Warning ID
    id: String,
    /// Player being warned
    player_id: u32,
    /// Moderator issuing warning
    moderator_id: u32,
    /// Reason
    reason: String,
    /// When warning was issued
    issued_at: DateTime<Utc>,
}

impl ChatModeration {
    /// Create new moderation system
    pub fn new() -> Self {
        Self {
            muted_players: HashMap::new(),
            banned_players: HashMap::new(),
            warnings: HashMap::new(),
            moderation_log: Vec::new(),
        }
    }

    /// Mute a player
    pub async fn mute_player(&mut self, player_id: u32, duration_seconds: Option<u64>) {
        let expires_at = duration_seconds.map(|secs| Utc::now() + Duration::seconds(secs as i64));

        let record = MuteRecord {
            player_id,
            moderator_id: 0, // System
            reason: "Muted".to_string(),
            issued_at: Utc::now(),
            expires_at,
        };

        self.muted_players.insert(player_id, record);

        info!("Muted player {} (duration: {:?})", player_id, duration_seconds);
    }

    /// Unmute a player
    pub async fn unmute_player(&mut self, player_id: u32) {
        self.muted_players.remove(&player_id);

        info!("Unmuted player {}", player_id);
    }

    /// Check if player is muted
    pub async fn is_player_muted(&mut self, player_id: u32) -> bool {
        // Check if an existing mute has expired
        let expired = match self.muted_players.get(&player_id) {
            Some(record) => match record.expires_at {
                Some(expires_at) => Utc::now() > expires_at,
                None => false,
            },
            None => return false,
        };

        if expired {
            self.unmute_player(player_id).await;
            return false;
        }

        true
    }

    /// Get mute record for player
    pub async fn get_mute_record(&self, player_id: u32) -> Option<MuteRecord> {
        self.muted_players.get(&player_id).cloned()
    }

    /// Ban a player
    pub async fn ban_player(&mut self, player_id: u32, moderator_id: u32, reason: String, duration_seconds: Option<u64>) {
        let expires_at = duration_seconds.map(|secs| Utc::now() + Duration::seconds(secs as i64));

        let record = BanRecord {
            player_id,
            moderator_id,
            reason: reason.clone(),
            issued_at: Utc::now(),
            expires_at,
        };

        self.banned_players.insert(player_id, record);

        // Log action
        self.log_moderation_action(ModerationAction {
            action: ModerationActionType::Ban,
            target_player: player_id,
            moderator: moderator_id,
            reason,
            duration_seconds,
            timestamp: Utc::now(),
        }).await;

        warn!("Banned player {} (duration: {:?})", player_id, duration_seconds);
    }

    /// Unban a player
    pub async fn unban_player(&mut self, player_id: u32, moderator_id: u32) {
        self.banned_players.remove(&player_id);

        // Log action
        self.log_moderation_action(ModerationAction {
            action: ModerationActionType::Unmute,
            target_player: player_id,
            moderator: moderator_id,
            reason: "Unbanned".to_string(),
            duration_seconds: None,
            timestamp: Utc::now(),
        }).await;

        info!("Unbanned player {}", player_id);
    }

    /// Check if player is banned
    pub async fn is_player_banned(&mut self, player_id: u32) -> bool {
        // Check if an existing ban has expired
        let expired = match self.banned_players.get(&player_id) {
            Some(record) => match record.expires_at {
                Some(expires_at) => Utc::now() > expires_at,
                None => false,
            },
            None => return false,
        };

        if expired {
            self.unban_player(player_id, 0).await;
            return false;
        }

        true
    }

    /// Get ban record for player
    pub async fn get_ban_record(&self, player_id: u32) -> Option<BanRecord> {
        self.banned_players.get(&player_id).cloned()
    }

    /// Warn a player
    pub async fn warn_player(&mut self, player_id: u32, moderator_id: u32, reason: String) {
        let warning = WarningRecord {
            id: uuid::Uuid::new_v4().to_string(),
            player_id,
            moderator_id,
            reason,
            issued_at: Utc::now(),
        };

        self.warnings
            .entry(player_id)
            .or_insert_with(Vec::new)
            .push(warning.clone());

        info!("Warned player {} for: {}", player_id, warning.reason);
    }

    /// Get warnings for player
    pub async fn get_player_warnings(&self, player_id: u32) -> Vec<WarningRecord> {
        self.warnings.get(&player_id).cloned().unwrap_or_default()
    }

    /// Get warning count for player
    pub async fn get_warning_count(&self, player_id: u32) -> usize {
        self.warnings.get(&player_id).map(|w| w.len()).unwrap_or(0)
    }

    /// Clear warnings for player
    pub async fn clear_warnings(&mut self, player_id: u32) {
        self.warnings.remove(&player_id);
    }

    /// Log moderation action
    async fn log_moderation_action(&mut self, action: ModerationAction) {
        self.moderation_log.push(action);

        // Keep log size manageable
        if self.moderation_log.len() > 1000 {
            self.moderation_log.drain(0..100);
        }
    }

    /// Get moderation log
    pub async fn get_moderation_log(&self, count: usize) -> Vec<ModerationAction> {
        self.moderation_log.iter().rev().take(count).cloned().collect()
    }

    /// Get all muted players
    pub async fn get_muted_players(&self) -> Vec<MuteRecord> {
        self.muted_players.values().cloned().collect()
    }

    /// Get all banned players
    pub async fn get_banned_players(&self) -> Vec<BanRecord> {
        self.banned_players.values().cloned().collect()
    }

    /// Clean up expired mutes and bans
    pub async fn cleanup_expired(&mut self) {
        let now = Utc::now();

        // Clean up expired mutes
        self.muted_players.retain(|_, record| {
            if let Some(expires_at) = record.expires_at {
                now <= expires_at
            } else {
                true // Permanent mutes stay
            }
        });

        // Clean up expired bans
        self.banned_players.retain(|_, record| {
            if let Some(expires_at) = record.expires_at {
                now <= expires_at
            } else {
                true // Permanent bans stay
            }
        });

        // Clean up old warnings (older than 30 days)
        let thirty_days_ago = now - Duration::days(30);
        for warnings_list in self.warnings.values_mut() {
            warnings_list.retain(|w| w.issued_at >= thirty_days_ago);
        }

        info!("Cleaned up expired moderation records");
    }

    /// Get moderation statistics
    pub async fn get_statistics(&self) -> ModerationStatistics {
        let total_warnings = self.warnings.values().map(|v| v.len()).sum();

        ModerationStatistics {
            active_mutes: self.muted_players.len(),
            active_bans: self.banned_players.len(),
            total_warnings,
        }
    }
}

impl Default for ChatModeration {
    fn default() -> Self {
        Self::new()
    }
}

/// Moderation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModerationStatistics {
    /// Number of active mutes
    pub active_mutes: usize,
    /// Number of active bans
    pub active_bans: usize,
    /// Total warnings issued
    pub total_warnings: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_moderation_creation() {
        let mut moderation = ChatModeration::new();

        assert!(!moderation.is_player_muted(1).await);
        assert!(!moderation.is_player_banned(1).await);
    }

    #[tokio::test]
    async fn test_mute_player() {
        let mut moderation = ChatModeration::new();

        moderation.mute_player(1, Some(60)).await;
        assert!(moderation.is_player_muted(1).await);
    }

    #[tokio::test]
    async fn test_unmute_player() {
        let mut moderation = ChatModeration::new();

        moderation.mute_player(1, None).await;
        assert!(moderation.is_player_muted(1).await);

        moderation.unmute_player(1).await;
        assert!(!moderation.is_player_muted(1).await);
    }

    #[tokio::test]
    async fn test_ban_player() {
        let mut moderation = ChatModeration::new();

        moderation.ban_player(1, 0, "Test ban".to_string(), None).await;
        assert!(moderation.is_player_banned(1).await);
    }

    #[tokio::test]
    async fn test_warn_player() {
        let mut moderation = ChatModeration::new();

        moderation.warn_player(1, 0, "Test warning".to_string()).await;

        let warnings = moderation.get_player_warnings(1).await;
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].reason, "Test warning");
    }

    #[tokio::test]
    async fn test_warning_count() {
        let mut moderation = ChatModeration::new();

        assert_eq!(moderation.get_warning_count(1).await, 0);

        moderation.warn_player(1, 0, "Warning 1".to_string()).await;
        moderation.warn_player(1, 0, "Warning 2".to_string()).await;

        assert_eq!(moderation.get_warning_count(1).await, 2);
    }

    #[tokio::test]
    async fn test_clear_warnings() {
        let mut moderation = ChatModeration::new();

        moderation.warn_player(1, 0, "Warning".to_string()).await;
        assert_eq!(moderation.get_warning_count(1).await, 1);

        moderation.clear_warnings(1).await;
        assert_eq!(moderation.get_warning_count(1).await, 0);
    }

    #[tokio::test]
    async fn test_statistics() {
        let mut moderation = ChatModeration::new();

        moderation.mute_player(1, None).await;
        moderation.mute_player(2, None).await;
        moderation.ban_player(3, 0, "Ban".to_string(), None).await;
        moderation.warn_player(4, 0, "Warning".to_string()).await;

        let stats = moderation.get_statistics().await;
        assert_eq!(stats.active_mutes, 2);
        assert_eq!(stats.active_bans, 1);
        assert_eq!(stats.total_warnings, 1);
    }
}
