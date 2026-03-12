//! KvPair Module
//! 
//! Corresponds to C++ file: Tools/CRCDiff/KVPair.cpp
//! 
//! This module provides key/value pair parsing functionality.

use std::collections::HashMap;
use std::fs;

/// Key/value mapping type
pub type KeyValueMap = HashMap<String, String>;

/// Key/Value Pair class for parsing configuration-like strings
pub struct KvPairClass {
    /// Internal key-value map
    map: KeyValueMap,
}

impl KvPairClass {
    /// Create new empty instance
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    
    /// Create and parse from input string with delimiter
    pub fn from_string(input: &str, delimiter: &str) -> Self {
        let mut instance = Self::new();
        instance.set(input, delimiter);
        instance
    }
    
    /// Set/parse from input string with delimiter
    pub fn set(&mut self, input: &str, delimiter: &str) {
        self.map = parse_into_kv_pairs(input, delimiter);
    }
    
    /// Read and parse from file with delimiter
    pub fn read_from_file(&mut self, filename: &str, delimiter: &str) -> Result<(), std::io::Error> {
        self.map.clear();
        let content = fs::read_to_string(filename)?;
        self.set(&content, delimiter);
        Ok(())
    }
    
    /// Get string value by key, returns empty string if not found
    pub fn get_string_val(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_default()
    }
    
    /// Get string value by key, returns Ok(value) if found
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }
    
    /// Get integer value by key
    pub fn get_int(&self, key: &str) -> Option<i32> {
        self.map.get(key)?.parse().ok()
    }
    
    /// Get unsigned integer value by key
    pub fn get_unsigned_int(&self, key: &str) -> Option<u32> {
        self.map.get(key)?.parse().ok()
    }
}

impl Default for KvPairClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert integer to string (utility function matching C++ intToString)
pub fn int_to_string(val: i32) -> String {
    val.to_string()
}

/// Trim whitespace and delimiter characters from string
fn trim_string(s: &str, delimiter: &str) -> String {
    let mut result = s.trim();
    
    // Remove delimiter characters from start and end
    while result.starts_with(delimiter) {
        result = &result[delimiter.len()..];
    }
    while result.ends_with(delimiter) {
        result = &result[..result.len() - delimiter.len()];
    }
    
    result.trim().to_string()
}

/// Parse input string into key-value pairs
fn parse_into_kv_pairs(input: &str, delimiter: &str) -> KeyValueMap {
    let mut map = HashMap::new();
    let mut remaining = input.to_string();
    
    while !remaining.is_empty() {
        let kv_pair = if let Some(delim_pos) = remaining.find(delimiter) {
            let pair = remaining[..delim_pos].to_string();
            remaining = remaining[delim_pos + delimiter.len()..].to_string();
            pair
        } else {
            let pair = remaining.clone();
            remaining.clear();
            pair
        };
        
        remaining = trim_string(&remaining, delimiter);
        let kv_trimmed = trim_string(&kv_pair, delimiter);
        
        if !kv_trimmed.is_empty() {
            if let Some(equals_pos) = kv_trimmed.find('=') {
                let key = trim_string(&trim_string(&kv_trimmed[..equals_pos], delimiter), " \t");
                let value = trim_string(&trim_string(&kv_trimmed[equals_pos + 1..], delimiter), " \t");
                
                if !key.is_empty() && !value.is_empty() {
                    map.insert(key, value);
                }
            }
        }
    }
    
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let kv = KvPairClass::from_string("key1=value1,key2=value2", ",");
        assert_eq!(kv.get_string_val("key1"), "value1");
        assert_eq!(kv.get_string_val("key2"), "value2");
        assert_eq!(kv.get_string_val("nonexistent"), "");
    }
    
    #[test]
    fn test_int_parsing() {
        let kv = KvPairClass::from_string("number=42,negative=-10", ",");
        assert_eq!(kv.get_int("number"), Some(42));
        assert_eq!(kv.get_int("negative"), Some(-10));
        assert_eq!(kv.get_int("nonexistent"), None);
    }
    
    #[test]
    fn test_unsigned_int_parsing() {
        let kv = KvPairClass::from_string("count=100", ",");
        assert_eq!(kv.get_unsigned_int("count"), Some(100));
        assert_eq!(kv.get_unsigned_int("nonexistent"), None);
    }
    
    #[test]
    fn test_whitespace_handling() {
        let kv = KvPairClass::from_string(" key1 = value1 , key2=value2", ",");
        assert_eq!(kv.get_string_val("key1"), "value1");
        assert_eq!(kv.get_string_val("key2"), "value2");
    }
    
    #[test]
    fn test_empty_values() {
        let kv = KvPairClass::from_string("key1=,key2=value2", ",");
        assert_eq!(kv.get_string_val("key2"), "value2");
        // Empty values should not be stored
        assert!(kv.get_string("key1").is_none());
    }
    
    #[test]
    fn test_different_delimiters() {
        let kv = KvPairClass::from_string("key1=value1;key2=value2", ";");
        assert_eq!(kv.get_string_val("key1"), "value1");
        assert_eq!(kv.get_string_val("key2"), "value2");
    }
    
    #[test]
    fn test_int_to_string_utility() {
        assert_eq!(int_to_string(42), "42");
        assert_eq!(int_to_string(-10), "-10");
        assert_eq!(int_to_string(0), "0");
    }
    
    #[test]
    fn test_trim_function() {
        assert_eq!(trim_string("  hello  ", " "), "hello");
        assert_eq!(trim_string(",,hello,,", ","), "hello");
        assert_eq!(trim_string(" , hello , ", ","), "hello");
    }
}
