//! Pattern Matching Module
//!
//! Provides pattern matching functionality equivalent to the C++ SimpleMatch function
//! for enabling/disabling profiling ranges based on wildcard patterns.

use crate::{ProfileError, ProfileResult};

/// Pattern matcher for profiling range names
pub struct PatternMatcher;

impl PatternMatcher {
    /// Simple pattern matcher equivalent to Profile::SimpleMatch
    /// Supports '*' as a wildcard character that matches any sequence of characters
    ///
    /// # Arguments
    /// * `text` - The text to match against
    /// * `pattern` - The pattern with optional '*' wildcards
    ///
    /// # Returns
    /// `true` if the text matches the pattern, `false` otherwise
    pub fn simple_match(text: &str, pattern: &str) -> bool {
        Self::match_recursive(text.chars().collect(), pattern.chars().collect(), 0, 0)
    }

    /// Recursive pattern matching implementation
    fn match_recursive(
        text: Vec<char>,
        pattern: Vec<char>,
        text_idx: usize,
        pattern_idx: usize,
    ) -> bool {
        // If we've consumed both strings, it's a match
        if pattern_idx >= pattern.len() && text_idx >= text.len() {
            return true;
        }

        // If pattern is consumed but text remains, no match (unless pattern ends with *)
        if pattern_idx >= pattern.len() {
            return false;
        }

        // If text is consumed but pattern has non-* characters remaining, no match
        if text_idx >= text.len() {
            // Check if remaining pattern is all '*'
            return pattern[pattern_idx..].iter().all(|&c| c == '*');
        }

        let current_pattern_char = pattern[pattern_idx];
        let current_text_char = text[text_idx];

        if current_pattern_char == '*' {
            // Try matching the wildcard with 0 or more characters
            // First, try matching 0 characters (skip the *)
            if Self::match_recursive(text.clone(), pattern.clone(), text_idx, pattern_idx + 1) {
                return true;
            }

            // Then, try matching 1 or more characters
            for i in (text_idx + 1)..=text.len() {
                if Self::match_recursive(text.clone(), pattern.clone(), i, pattern_idx + 1) {
                    return true;
                }
            }

            false
        } else {
            // Exact character match required
            if current_text_char == current_pattern_char {
                Self::match_recursive(text, pattern, text_idx + 1, pattern_idx + 1)
            } else {
                false
            }
        }
    }

    /// Check if a pattern is valid (contains only allowed characters)
    pub fn is_valid_pattern(pattern: &str) -> bool {
        // Allow alphanumeric characters, dots, underscores, and wildcards
        pattern
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '*')
    }

    /// Normalize a pattern by removing redundant wildcards
    pub fn normalize_pattern(pattern: &str) -> String {
        let mut result = String::new();
        let mut last_was_star = false;

        for ch in pattern.chars() {
            if ch == '*' {
                if !last_was_star {
                    result.push(ch);
                    last_was_star = true;
                }
            } else {
                result.push(ch);
                last_was_star = false;
            }
        }

        result
    }
}

/// A collection of patterns with their active/inactive states
#[derive(Debug, Clone)]
pub struct PatternList {
    patterns: Vec<PatternEntry>,
}

#[derive(Debug, Clone)]
pub struct PatternEntry {
    pub pattern: String,
    pub is_active: bool,
}

impl PatternList {
    /// Create a new empty pattern list
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Add a pattern to the list
    pub fn add_pattern(&mut self, pattern: &str, is_active: bool) -> ProfileResult<()> {
        if !PatternMatcher::is_valid_pattern(pattern) {
            return Err(ProfileError::PatternError(format!(
                "Invalid pattern: {}",
                pattern
            )));
        }

        let normalized = PatternMatcher::normalize_pattern(pattern);

        self.patterns.push(PatternEntry {
            pattern: normalized,
            is_active,
        });

        Ok(())
    }

    /// Clear all patterns
    pub fn clear(&mut self) {
        self.patterns.clear();
    }

    /// Check if a text matches any pattern and return the final active state
    /// Later patterns override earlier ones for the same text
    pub fn is_active(&self, text: &str) -> Option<bool> {
        let mut result = None;

        for entry in &self.patterns {
            if PatternMatcher::simple_match(text, &entry.pattern) {
                result = Some(entry.is_active);
            }
        }

        result
    }

    /// Get all patterns
    pub fn get_patterns(&self) -> &[PatternEntry] {
        &self.patterns
    }

    /// Get pattern count
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Remove patterns that match a given text
    pub fn remove_matching(&mut self, text: &str) {
        self.patterns
            .retain(|entry| !PatternMatcher::simple_match(text, &entry.pattern));
    }

    /// Get active patterns (those that enable profiling)
    pub fn get_active_patterns(&self) -> Vec<&str> {
        self.patterns
            .iter()
            .filter(|entry| entry.is_active)
            .map(|entry| entry.pattern.as_str())
            .collect()
    }

    /// Get inactive patterns (those that disable profiling)
    pub fn get_inactive_patterns(&self) -> Vec<&str> {
        self.patterns
            .iter()
            .filter(|entry| !entry.is_active)
            .map(|entry| entry.pattern.as_str())
            .collect()
    }
}

impl Default for PatternList {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PatternList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.patterns.is_empty() {
            writeln!(f, "No patterns defined")?;
        } else {
            writeln!(f, "Patterns ({} total):", self.patterns.len())?;
            for (i, entry) in self.patterns.iter().enumerate() {
                writeln!(
                    f,
                    "  {}: {} {}",
                    i + 1,
                    if entry.is_active { "+" } else { "-" },
                    entry.pattern
                )?;
            }
        }
        Ok(())
    }
}

/// Pattern-based profiling controller
pub struct PatternBasedProfiler {
    pattern_list: PatternList,
}

impl PatternBasedProfiler {
    /// Create a new pattern-based profiler
    pub fn new() -> Self {
        Self {
            pattern_list: PatternList::new(),
        }
    }

    /// Add a pattern for enabling profiling
    pub fn enable_pattern(&mut self, pattern: &str) -> ProfileResult<()> {
        self.pattern_list.add_pattern(pattern, true)
    }

    /// Add a pattern for disabling profiling
    pub fn disable_pattern(&mut self, pattern: &str) -> ProfileResult<()> {
        self.pattern_list.add_pattern(pattern, false)
    }

    /// Check if profiling should be enabled for a given name
    pub fn should_profile(&self, name: &str) -> bool {
        // By default, profiling is disabled unless explicitly enabled
        self.pattern_list.is_active(name).unwrap_or(false)
    }

    /// Clear all patterns
    pub fn clear_patterns(&mut self) {
        self.pattern_list.clear();
    }

    /// Get the pattern list
    pub fn get_pattern_list(&self) -> &PatternList {
        &self.pattern_list
    }

    /// Get a mutable reference to the pattern list
    pub fn get_pattern_list_mut(&mut self) -> &mut PatternList {
        &mut self.pattern_list
    }

    /// Load patterns from a configuration string
    /// Format: "+pattern1 -pattern2 +pattern3"
    pub fn load_from_config(&mut self, config: &str) -> ProfileResult<()> {
        self.clear_patterns();

        for part in config.split_whitespace() {
            if let Some(pattern) = part.strip_prefix('+') {
                self.enable_pattern(pattern)?;
            } else if let Some(pattern) = part.strip_prefix('-') {
                self.disable_pattern(pattern)?;
            } else if !part.is_empty() {
                return Err(ProfileError::PatternError(format!(
                    "Pattern must start with + or -: {}",
                    part
                )));
            }
        }

        Ok(())
    }

    /// Save patterns to a configuration string
    pub fn save_to_config(&self) -> String {
        let mut config = String::new();

        for entry in self.pattern_list.get_patterns() {
            if !config.is_empty() {
                config.push(' ');
            }
            config.push(if entry.is_active { '+' } else { '-' });
            config.push_str(&entry.pattern);
        }

        config
    }
}

impl Default for PatternBasedProfiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_match_exact() {
        assert!(PatternMatcher::simple_match("hello", "hello"));
        assert!(!PatternMatcher::simple_match("hello", "world"));
        assert!(!PatternMatcher::simple_match("hello", "hell"));
        assert!(!PatternMatcher::simple_match("hell", "hello"));
    }

    #[test]
    fn test_simple_match_wildcard() {
        // Single wildcard
        assert!(PatternMatcher::simple_match("hello", "*"));
        assert!(PatternMatcher::simple_match("", "*"));

        // Wildcard at end
        assert!(PatternMatcher::simple_match("hello", "hel*"));
        assert!(PatternMatcher::simple_match("hello", "hello*"));
        assert!(!PatternMatcher::simple_match("hello", "world*"));

        // Wildcard at start
        assert!(PatternMatcher::simple_match("hello", "*llo"));
        assert!(PatternMatcher::simple_match("hello", "*hello"));
        assert!(!PatternMatcher::simple_match("hello", "*world"));

        // Wildcard in middle
        assert!(PatternMatcher::simple_match("hello", "he*lo"));
        assert!(PatternMatcher::simple_match("hello", "h*o"));
        assert!(!PatternMatcher::simple_match("hello", "he*world"));
    }

    #[test]
    fn test_simple_match_multiple_wildcards() {
        assert!(PatternMatcher::simple_match("hello.world.test", "*.*.*"));
        assert!(PatternMatcher::simple_match("hello.world", "*.*"));
        assert!(PatternMatcher::simple_match("a.b.c.d", "a.*.*"));
        assert!(!PatternMatcher::simple_match("hello", "*.*"));
    }

    #[test]
    fn test_simple_match_edge_cases() {
        // Empty strings
        assert!(PatternMatcher::simple_match("", ""));
        assert!(PatternMatcher::simple_match("", "*"));
        assert!(!PatternMatcher::simple_match("hello", ""));

        // Multiple consecutive wildcards (should work the same as single wildcard)
        assert!(PatternMatcher::simple_match("hello", "**"));
        assert!(PatternMatcher::simple_match("hello", "h**o"));
    }

    #[test]
    fn test_pattern_validation() {
        assert!(PatternMatcher::is_valid_pattern("hello.world"));
        assert!(PatternMatcher::is_valid_pattern("test_123"));
        assert!(PatternMatcher::is_valid_pattern("*"));
        assert!(PatternMatcher::is_valid_pattern("test.*"));

        // Invalid characters (this is a design choice - modify as needed)
        assert!(!PatternMatcher::is_valid_pattern("test/path"));
        assert!(!PatternMatcher::is_valid_pattern("test space"));
    }

    #[test]
    fn test_pattern_normalization() {
        assert_eq!(PatternMatcher::normalize_pattern("hello"), "hello");
        assert_eq!(PatternMatcher::normalize_pattern("*"), "*");
        assert_eq!(PatternMatcher::normalize_pattern("**"), "*");
        assert_eq!(PatternMatcher::normalize_pattern("***hello***"), "*hello*");
        assert_eq!(PatternMatcher::normalize_pattern("a**b**c"), "a*b*c");
    }

    #[test]
    fn test_pattern_list() {
        let mut list = PatternList::new();
        assert!(list.is_empty());

        list.add_pattern("test.*", true).unwrap();
        list.add_pattern("debug.*", false).unwrap();

        assert_eq!(list.len(), 2);

        // Test matching
        assert_eq!(list.is_active("test.function"), Some(true));
        assert_eq!(list.is_active("debug.info"), Some(false));
        assert_eq!(list.is_active("other.function"), None);

        // Test override (later patterns win)
        list.add_pattern("test.slow", false).unwrap();
        assert_eq!(list.is_active("test.slow"), Some(false)); // Overrides test.*
        assert_eq!(list.is_active("test.fast"), Some(true)); // Still matches test.*
    }

    #[test]
    fn test_pattern_based_profiler() {
        let mut profiler = PatternBasedProfiler::new();

        // Initially, nothing should be profiled
        assert!(!profiler.should_profile("anything"));

        // Add some patterns
        profiler.enable_pattern("render.*").unwrap();
        profiler.enable_pattern("audio.*").unwrap();
        profiler.disable_pattern("audio.background").unwrap();

        assert!(profiler.should_profile("render.textures"));
        assert!(profiler.should_profile("audio.effects"));
        assert!(!profiler.should_profile("audio.background")); // Disabled by specific pattern
        assert!(!profiler.should_profile("game.logic")); // Not enabled
    }

    #[test]
    fn test_config_loading() {
        let mut profiler = PatternBasedProfiler::new();

        profiler
            .load_from_config("+render.* +audio.* -audio.background")
            .unwrap();

        assert!(profiler.should_profile("render.textures"));
        assert!(profiler.should_profile("audio.effects"));
        assert!(!profiler.should_profile("audio.background"));
        assert!(!profiler.should_profile("game.logic"));

        let config = profiler.save_to_config();
        assert!(config.contains("+render.*"));
        assert!(config.contains("+audio.*"));
        assert!(config.contains("-audio.background"));
    }

    #[test]
    fn test_config_errors() {
        let mut profiler = PatternBasedProfiler::new();

        // Missing + or - prefix
        let result = profiler.load_from_config("render.*");
        assert!(result.is_err());

        // Invalid pattern characters
        let result = profiler.enable_pattern("test/invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_pattern_list_display() {
        let mut list = PatternList::new();
        list.add_pattern("test.*", true).unwrap();
        list.add_pattern("debug.*", false).unwrap();

        let display = format!("{}", list);
        assert!(display.contains("+ test.*"));
        assert!(display.contains("- debug.*"));
        assert!(display.contains("2 total"));
    }

    #[test]
    fn test_pattern_removal() {
        let mut list = PatternList::new();
        list.add_pattern("test.*", true).unwrap();
        list.add_pattern("debug.*", false).unwrap();
        list.add_pattern("temp.*", true).unwrap();

        assert_eq!(list.len(), 3);

        // Remove patterns matching "temp.something"
        list.remove_matching("temp.something");
        assert_eq!(list.len(), 2);

        // Should still have test.* and debug.*
        assert!(list.is_active("test.func").is_some());
        assert!(list.is_active("debug.info").is_some());
        assert!(list.is_active("temp.func").is_none());
    }

    #[test]
    fn test_get_active_inactive_patterns() {
        let mut list = PatternList::new();
        list.add_pattern("active1", true).unwrap();
        list.add_pattern("inactive1", false).unwrap();
        list.add_pattern("active2", true).unwrap();

        let active = list.get_active_patterns();
        let inactive = list.get_inactive_patterns();

        assert_eq!(active.len(), 2);
        assert_eq!(inactive.len(), 1);
        assert!(active.contains(&"active1"));
        assert!(active.contains(&"active2"));
        assert!(inactive.contains(&"inactive1"));
    }
}
