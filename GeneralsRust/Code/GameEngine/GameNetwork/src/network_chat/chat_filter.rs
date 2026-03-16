//! Chat Filter Module
//!
//! Provides profanity filtering and spam prevention for chat messages

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Chat message filter
#[derive(Clone)]
pub struct ChatFilter {
    /// Profanity word list
    profanity_words: Arc<RwLock<HashSet<String>>>,
    /// Spam detection
    spam_detector: Arc<RwLock<SpamDetector>>,
    /// Whether filtering is enabled
    filter_enabled: Arc<RwLock<bool>>,
    /// Replacement character
    replacement_char: Arc<RwLock<char>>,
}

/// Spam detector for preventing message spam
#[derive(Debug)]
pub struct SpamDetector {
    /// Message history for duplicate detection
    message_history: VecDeque<String>,
    /// Timestamp history for rate limiting
    timestamp_history: VecDeque<Instant>,
    /// Maximum duplicate messages allowed
    max_duplicates: usize,
    /// Maximum messages per time window
    max_messages_per_window: usize,
    /// Time window for rate limiting
    rate_limit_window: Duration,
    /// Minimum time between messages
    min_message_interval: Duration,
}

impl SpamDetector {
    /// Create new spam detector
    pub fn new() -> Self {
        Self {
            message_history: VecDeque::with_capacity(10),
            timestamp_history: VecDeque::with_capacity(20),
            max_duplicates: 3,
            max_messages_per_window: 10,
            rate_limit_window: Duration::from_secs(30),
            min_message_interval: Duration::from_millis(500),
        }
    }

    /// Check if message should be blocked as spam
    pub fn is_spam(&mut self, message: &str) -> bool {
        let now = Instant::now();

        // Clean old timestamps
        while let Some(&front_time) = self.timestamp_history.front() {
            if now.duration_since(front_time) > self.rate_limit_window {
                self.timestamp_history.pop_front();
            } else {
                break;
            }
        }

        // Check rate limit
        if self.timestamp_history.len() >= self.max_messages_per_window {
            return true;
        }

        // Check minimum interval
        if let Some(&last_time) = self.timestamp_history.back() {
            if now.duration_since(last_time) < self.min_message_interval {
                return true;
            }
        }

        // Check for duplicate messages
        let duplicate_count = self.message_history.iter()
            .filter(|msg| msg.to_lowercase() == message.to_lowercase())
            .count();

        if duplicate_count >= self.max_duplicates {
            return true;
        }

        // Add to history
        self.message_history.push_back(message.to_string());
        if self.message_history.len() > 10 {
            self.message_history.pop_front();
        }

        self.timestamp_history.push_back(now);

        false
    }

    /// Reset spam detector state
    pub fn reset(&mut self) {
        self.message_history.clear();
        self.timestamp_history.clear();
    }
}

impl Default for SpamDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatFilter {
    /// Create new chat filter
    pub fn new() -> Self {
        let filter = Self {
            profanity_words: Arc::new(RwLock::new(HashSet::new())),
            spam_detector: Arc::new(RwLock::new(SpamDetector::new())),
            filter_enabled: Arc::new(RwLock::new(true)),
            replacement_char: Arc::new(RwLock::new('*')),
        };

        // Initialize default profanity list
        let filter_clone = filter.clone();
        tokio::spawn(async move {
            filter_clone.initialize_default_profanity_list().await;
        });

        filter
    }

    /// Initialize default profanity word list
    async fn initialize_default_profanity_list(&self) {
        let default_words = vec![
            // Add default profanity words here
            // This is a placeholder - real implementation would have comprehensive list
            "badword1".to_string(),
            "badword2".to_string(),
            "badword3".to_string(),
        ];

        let mut profanity = self.profanity_words.write().await;
        for word in default_words {
            profanity.insert(word.to_lowercase());
        }
    }

    /// Filter a chat message
    /// Returns (filtered_message, was_filtered)
    pub fn filter_message(&self, message: &str) -> (String, bool) {
        // Note: This is a synchronous version for compatibility
        // In real implementation, this should be async

        let filtered = self.apply_profanity_filter(message);
        let was_filtered = filtered != message;

        (filtered, was_filtered)
    }

    /// Apply profanity filter to message
    fn apply_profanity_filter(&self, message: &str) -> String {
        let mut filtered = message.to_string();

        // Simple word-based filtering
        // In production, use more sophisticated methods
        let profanity_words = vec![
            "badword1", "badword2", "badword3",
        ];

        for word in &profanity_words {
            let replacement = "*".repeat(word.len());
            filtered = filtered.replace(word, &replacement);
            filtered = filtered.replace(&word.to_uppercase(), &replacement);
            filtered = filtered.replace(&word.to_lowercase(), &replacement);
        }

        filtered
    }

    /// Check if message is spam
    pub async fn is_spam(&self, message: &str) -> bool {
        let mut detector = self.spam_detector.write().await;
        detector.is_spam(message)
    }

    /// Add profanity word to filter
    pub async fn add_profanity_word(&self, word: String) {
        let mut profanity = self.profanity_words.write().await;
        profanity.insert(word.to_lowercase());
    }

    /// Remove profanity word from filter
    pub async fn remove_profanity_word(&self, word: &str) {
        let mut profanity = self.profanity_words.write().await;
        profanity.remove(&word.to_lowercase());
    }

    /// Enable or disable filtering
    pub async fn set_enabled(&self, enabled: bool) {
        *self.filter_enabled.write().await = enabled;
    }

    /// Check if filtering is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.filter_enabled.read().await
    }

    /// Set replacement character
    pub async fn set_replacement_char(&self, ch: char) {
        *self.replacement_char.write().await = ch;
    }

    /// Reset spam detector
    pub async fn reset_spam_detector(&self) {
        let mut detector = self.spam_detector.write().await;
        detector.reset();
    }

    /// Validate message before sending
    pub async fn validate_message(&self, message: &str) -> Result<(), String> {
        // Check empty
        if message.trim().is_empty() {
            return Err("Message cannot be empty".to_string());
        }

        // Check length
        if message.len() > crate::network_chat::MAX_MESSAGE_LENGTH {
            return Err(format!("Message too long: {} > {}", message.len(), crate::network_chat::MAX_MESSAGE_LENGTH));
        }

        // Check spam
        if self.is_spam(message).await {
            return Err("Message detected as spam".to_string());
        }

        Ok(())
    }
}

impl Default for ChatFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spam_detector_creation() {
        let detector = SpamDetector::new();
        assert!(!detector.is_spam("test message"));
    }

    #[test]
    fn test_duplicate_detection() {
        let mut detector = SpamDetector::new();

        // Send same message 4 times (exceeds limit of 3)
        for _ in 0..4 {
            let result = detector.is_spam("duplicate message");
            if _ < 3 {
                assert!(!result, "Should not be spam on attempt {}", _ + 1);
            } else {
                assert!(result, "Should be spam on attempt {}", _ + 1);
            }
        }
    }

    #[test]
    fn test_rate_limiting() {
        let mut detector = SpamDetector::new();

        // Send many messages quickly
        let mut spam_count = 0;
        for i in 0..15 {
            if detector.is_spam(&format!("message {}", i)) {
                spam_count += 1;
            }
        }

        // Should trigger spam after ~10 messages
        assert!(spam_count > 0, "Should detect spam");
    }

    #[tokio::test]
    async fn test_filter_creation() {
        let filter = ChatFilter::new();
        assert!(filter.is_enabled().await);
    }

    #[tokio::test]
    async fn test_message_validation() {
        let filter = ChatFilter::new();

        // Empty message
        assert!(filter.validate_message("").await.is_err());

        // Valid message
        assert!(filter.validate_message("Hello world").await.is_ok());
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let filter = ChatFilter::new();

        filter.set_enabled(false).await;
        assert!(!filter.is_enabled().await);

        filter.set_enabled(true).await;
        assert!(filter.is_enabled().await);
    }
}
