//! Chat History Module
//!
//! Manages chat message history with persistence and search capabilities

use crate::network_chat::UnifiedChatMessage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Chat history manager
pub struct ChatHistoryManager {
    /// In-memory message history
    history: Arc<RwLock<VecDeque<HistoryEntry>>>,
    /// Maximum messages to keep in memory
    max_memory_messages: usize,
    /// History persistence
    persistence: Arc<RwLock<HistoryPersistence>>,
}

/// History entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    /// The message
    message: UnifiedChatMessage,
    /// When it was added to history
    recorded_at: DateTime<Utc>,
    /// Session ID (for grouping)
    session_id: String,
}

/// History persistence configuration
#[derive(Debug, Clone)]
struct HistoryPersistence {
    /// Whether persistence is enabled
    enabled: bool,
    /// Path to history file
    history_file: PathBuf,
    /// Auto-save interval in seconds
    auto_save_interval: u64,
    /// Maximum file size in bytes
    max_file_size: usize,
}

impl Default for HistoryPersistence {
    fn default() -> Self {
        Self {
            enabled: true,
            history_file: PathBuf::from("chat_history.jsonl"),
            auto_save_interval: 60,
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl ChatHistoryManager {
    /// Create new chat history manager
    pub fn new(max_messages: usize) -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(max_messages))),
            max_memory_messages: max_messages,
            persistence: Arc::new(RwLock::new(HistoryPersistence::default())),
        }
    }

    /// Initialize history manager
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing chat history manager");

        // Load existing history from disk
        self.load_from_disk().await?;

        info!("Chat history manager initialized");
        Ok(())
    }

    /// Add message to history
    pub async fn add_message(&self, message: UnifiedChatMessage) {
        let entry = HistoryEntry {
            message,
            recorded_at: Utc::now(),
            session_id: self.get_session_id(),
        };

        let mut history = self.history.write().await;

        // Add to history
        history.push_back(entry);

        // Trim if necessary
        while history.len() > self.max_memory_messages {
            history.pop_front();
        }

        debug!("Added message to history (total: {})", history.len());
    }

    /// Get recent messages
    pub async fn get_recent(&self, count: usize) -> Vec<UnifiedChatMessage> {
        let history = self.history.read().await;

        history
            .iter()
            .rev()
            .take(count)
            .map(|entry| entry.message.clone())
            .collect()
    }

    /// Get messages since timestamp
    pub async fn get_messages_since(&self, since: DateTime<Utc>) -> Vec<UnifiedChatMessage> {
        let history = self.history.read().await;

        history
            .iter()
            .filter(|entry| entry.recorded_at >= since)
            .map(|entry| entry.message.clone())
            .collect()
    }

    /// Search messages by content
    pub async fn search_messages(&self, query: &str) -> Vec<UnifiedChatMessage> {
        let history = self.history.read().await;
        let query_lower = query.to_lowercase();

        history
            .iter()
            .filter(|entry| {
                entry.message.message.to_lowercase().contains(&query_lower) ||
                entry.message.sender_name.to_lowercase().contains(&query_lower)
            })
            .map(|entry| entry.message.clone())
            .collect()
    }

    /// Get messages from specific player
    pub async fn get_messages_from_player(&self, player_id: u32) -> Vec<UnifiedChatMessage> {
        let history = self.history.read().await;

        history
            .iter()
            .filter(|entry| entry.message.sender_id == player_id)
            .map(|entry| entry.message.clone())
            .collect()
    }

    /// Get messages from specific channel
    pub async fn get_messages_from_channel(&self, channel: &crate::network_chat::ChatChannel) -> Vec<UnifiedChatMessage> {
        let history = self.history.read().await;

        history
            .iter()
            .filter(|entry| entry.message.channel == *channel)
            .map(|entry| entry.message.clone())
            .collect()
    }

    /// Clear all history
    pub async fn clear(&self) {
        let mut history = self.history.write().await;
        history.clear();
        info!("Chat history cleared");
    }

    /// Save history to disk
    pub async fn save_to_disk(&self) -> Result<(), Box<dyn std::error::Error>> {
        let persistence = self.persistence.read().await;

        if !persistence.enabled {
            return Ok(());
        }

        info!("Saving chat history to disk");

        let history = self.history.read().await;

        // Create backup if file exists and is large
        if persistence.history_file.exists() {
            let metadata = std::fs::metadata(&persistence.history_file)?;
            if metadata.len() as usize > persistence.max_file_size {
                self.rotate_history_file().await?;
            }
        }

        // Open file for writing
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&persistence.history_file)?;

        let mut writer = BufWriter::new(file);

        // Write each entry as JSONL
        for entry in history.iter() {
            let json = serde_json::to_string(entry)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;

        info!("Saved {} messages to disk", history.len());
        Ok(())
    }

    /// Load history from disk
    async fn load_from_disk(&self) -> Result<(), Box<dyn std::error::Error>> {
        let persistence = self.persistence.read().await;

        if !persistence.enabled || !persistence.history_file.exists() {
            return Ok(());
        }

        info!("Loading chat history from disk");

        let file = File::open(&persistence.history_file)?;
        let reader = BufReader::new(file);
        let mut loaded_count = 0;

        let mut history = self.history.write().await;

        for line in reader.lines() {
            match line {
                Ok(json) => {
                    if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&json) {
                        history.push_back(entry);
                        loaded_count += 1;
                    }
                }
                Err(e) => {
                    warn!("Error reading history line: {}", e);
                }
            }

            // Limit loading
            if history.len() >= self.max_memory_messages {
                break;
            }
        }

        info!("Loaded {} messages from disk", loaded_count);
        Ok(())
    }

    /// Rotate history file (backup and create new)
    async fn rotate_history_file(&self) -> Result<(), Box<dyn std::error::Error>> {
        let persistence = self.persistence.read().await;

        if !persistence.history_file.exists() {
            return Ok(());
        }

        // Create backup filename with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = format!("{}.backup_{}", persistence.history_file.display(), timestamp);

        info!("Rotating history file to {}", backup_path);

        // Rename current file to backup
        std::fs::rename(&persistence.history_file, &backup_path)?;

        Ok(())
    }

    /// Configure persistence
    pub async fn configure_persistence(&self, enabled: bool, history_file: PathBuf) {
        let mut persistence = self.persistence.write().await;
        persistence.enabled = enabled;
        persistence.history_file = history_file;
        info!("Persistence configured: enabled={}, file={:?}", enabled, persistence.history_file);
    }

    /// Get current session ID
    fn get_session_id(&self) -> String {
        // Generate session ID based on date/time
        Utc::now().format("%Y%m%d").to_string()
    }

    /// Get history statistics
    pub async fn get_statistics(&self) -> HistoryStatistics {
        let history = self.history.read().await;

        let mut player_counts = std::collections::HashMap::new();
        let mut channel_counts = std::collections::HashMap::new();

        for entry in history.iter() {
            *player_counts.entry(entry.message.sender_id).or_insert(0) += 1;
            *channel_counts.entry(format!("{:?}", entry.message.channel)).or_insert(0) += 1;
        }

        HistoryStatistics {
            total_messages: history.len(),
            unique_players: player_counts.len(),
            most_active_player: player_counts.into_iter()
                .max_by_key(|&(_, count)| count)
                .map(|(id, count)| (id, count)),
            channel_distribution: channel_counts,
            oldest_message: history.front().map(|e| e.recorded_at),
            newest_message: history.back().map(|e| e.recorded_at),
        }
    }
}

/// History statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStatistics {
    /// Total messages in history
    pub total_messages: usize,
    /// Number of unique players
    pub unique_players: usize,
    /// Most active player (ID, message count)
    pub most_active_player: Option<(u32, usize)>,
    /// Message count per channel
    pub channel_distribution: std::collections::HashMap<String, usize>,
    /// Oldest message timestamp
    pub oldest_message: Option<DateTime<Utc>>,
    /// Newest message timestamp
    pub newest_message: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_chat::{ChatChannel, ChatMessageType};

    #[tokio::test]
    async fn test_history_creation() {
        let manager = ChatHistoryManager::new(100);
        let recent = manager.get_recent(10).await;
        assert!(recent.is_empty());
    }

    #[tokio::test]
    async fn test_add_message() {
        let manager = ChatHistoryManager::new(100);

        let message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 1,
            sender_name: "TestPlayer".to_string(),
            message: "Test message".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        manager.add_message(message).await;

        let recent = manager.get_recent(10).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "Test message");
    }

    #[tokio::test]
    async fn test_search_messages() {
        let manager = ChatHistoryManager::new(100);

        let message1 = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 1,
            sender_name: "PlayerOne".to_string(),
            message: "Hello world".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        let message2 = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 2,
            sender_name: "PlayerTwo".to_string(),
            message: "Goodbye world".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        manager.add_message(message1).await;
        manager.add_message(message2).await;

        let results = manager.search_messages("world").await;
        assert_eq!(results.len(), 2);

        let results = manager.search_messages("PlayerOne").await;
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let manager = ChatHistoryManager::new(100);

        let message = UnifiedChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            sender_id: 1,
            sender_name: "TestPlayer".to_string(),
            message: "Test message".to_string(),
            channel: ChatChannel::Global,
            message_type: ChatMessageType::Normal,
            timestamp: Utc::now(),
            target_player: None,
            emoticon: None,
            was_filtered: false,
            original_message: None,
        };

        manager.add_message(message).await;
        manager.clear().await;

        let recent = manager.get_recent(10).await;
        assert!(recent.is_empty());
    }

    #[tokio::test]
    async fn test_statistics() {
        let manager = ChatHistoryManager::new(100);

        for i in 1..=3 {
            let message = UnifiedChatMessage {
                id: uuid::Uuid::new_v4().to_string(),
                sender_id: i,
                sender_name: format!("Player{}", i),
                message: format!("Message {}", i),
                channel: ChatChannel::Global,
                message_type: ChatMessageType::Normal,
                timestamp: Utc::now(),
                target_player: None,
                emoticon: None,
                was_filtered: false,
                original_message: None,
            };
            manager.add_message(message).await;
        }

        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.unique_players, 3);
    }
}
