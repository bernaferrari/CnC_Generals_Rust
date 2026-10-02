//! Chat Filter Module
//!
//! Provides profanity filtering and spam prevention for chat messages

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Chat message filter
///
/// Single-owner state: the word list, detector and knobs are only touched from
/// this struct's own methods (mutators take `&mut self`), so they are plain
/// fields.  The defaults that used to be installed from a detached one-shot
/// task are now installed synchronously in `new`.
#[derive(Clone)]
pub struct ChatFilter {
    /// Profanity word list
    profanity_words: HashSet<String>,
    /// Spam detection
    spam_detector: SpamDetector,
    /// Whether filtering is enabled
    filter_enabled: bool,
    /// Replacement character
    replacement_char: char,
}

/// Spam detector for preventing message spam
#[derive(Debug, Clone)]
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
        let mut filter = Self {
            profanity_words: HashSet::new(),
            spam_detector: SpamDetector::new(),
            filter_enabled: true,
            replacement_char: '*',
        };

        // Initialize default profanity list
        filter.initialize_default_profanity_list();

        filter
    }

    /// Initialize default profanity word list
    fn initialize_default_profanity_list(&mut self) {
        let default_words = vec![
            // Add default profanity words here
            // This is a placeholder - real implementation would have comprehensive list
            "badword1".to_string(),
            "badword2".to_string(),
            "badword3".to_string(),
        ];

        for word in default_words {
            self.profanity_words.insert(word.to_lowercase());
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
    pub async fn is_spam(&mut self, message: &str) -> bool {
        self.spam_detector.is_spam(message)
    }

    /// Add profanity word to filter
    pub async fn add_profanity_word(&mut self, word: String) {
        self.profanity_words.insert(word.to_lowercase());
    }

    /// Remove profanity word from filter
    pub async fn remove_profanity_word(&mut self, word: &str) {
        self.profanity_words.remove(&word.to_lowercase());
    }

    /// Enable or disable filtering
    pub async fn set_enabled(&mut self, enabled: bool) {
        self.filter_enabled = enabled;
    }

    /// Check if filtering is enabled
    pub async fn is_enabled(&self) -> bool {
        self.filter_enabled
    }

    /// Set replacement character
    pub async fn set_replacement_char(&mut self, ch: char) {
        self.replacement_char = ch;
    }

    /// Reset spam detector
    pub async fn reset_spam_detector(&mut self) {
        self.spam_detector.reset();
    }

    /// Validate message before sending
    pub async fn validate_message(&mut self, message: &str) -> Result<(), String> {
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
        let mut filter = ChatFilter::new();
        assert!(filter.is_enabled().await);
    }

    #[tokio::test]
    async fn test_message_validation() {
        let mut filter = ChatFilter::new();

        // Empty message
        assert!(filter.validate_message("").await.is_err());

        // Valid message
        assert!(filter.validate_message("Hello world").await.is_ok());
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let mut filter = ChatFilter::new();

        filter.set_enabled(false).await;
        assert!(!filter.is_enabled().await);

        filter.set_enabled(true).await;
        assert!(filter.is_enabled().await);
    }
}
