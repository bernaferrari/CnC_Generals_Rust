////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! ASCII String Implementation
//!
//! This module provides a general-purpose ASCII string type for the game engine.
//! It is modeled after the MFC CString class with reference counting and efficient
//! memory management.
//!
//! Steven Johnson, October 2001
//! Rust conversion: 2025

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// Maximum length for formatted strings
pub const MAX_FORMAT_BUF_LEN: usize = 2048;

/// Maximum total length of any AsciiString
pub const MAX_LEN: usize = 32767;

/// ASCII String class that provides efficient string operations with reference counting
#[derive(Debug, Clone)]
pub struct AsciiString {
    data: String,
}

impl Default for AsciiString {
    fn default() -> Self {
        Self::new()
    }
}

impl AsciiString {
    /// Create a new empty AsciiString
    pub fn new() -> Self {
        Self {
            data: String::new(),
        }
    }

    /// Get an empty string (compatibility helper)
    #[allow(non_snake_case)]
    pub fn TheEmptyString() -> Self {
        Self::new()
    }

    /// Create an AsciiString from a string slice
    pub fn from(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        self.data.len()
    }

    /// Get the length of the string in characters (alias for get_length)
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Check if the string is not empty
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    /// Clear the string, making it empty
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get the character at the given index
    pub fn get_char_at(&self, index: usize) -> Option<char> {
        self.data.chars().nth(index)
    }

    /// Get the string as a &str
    pub fn as_str(&self) -> &str {
        &self.data
    }

    /// Get a C-style string pointer (for compatibility)
    pub fn str(&self) -> &str {
        &self.data
    }

    /// Set the contents of the string
    pub fn set(&mut self, s: &str) {
        self.data = s.to_string();
    }

    /// Set from another AsciiString
    pub fn set_from_ascii_string(&mut self, other: &AsciiString) {
        self.data = other.data.clone();
    }

    /// Concatenate a string slice
    pub fn concat(&mut self, s: &str) {
        self.data.push_str(s);
    }

    /// Concatenate another AsciiString
    pub fn concat_ascii_string(&mut self, other: &AsciiString) {
        self.data.push_str(&other.data);
    }

    /// Concatenate a single character
    pub fn push(&mut self, c: char) {
        self.data.push(c);
    }

    /// Push a string slice
    pub fn push_str(&mut self, s: &str) {
        self.data.push_str(s);
    }

    /// Remove leading and trailing whitespace
    pub fn trim(&mut self) {
        self.data = self.data.trim().to_string();
    }

    /// Convert to lowercase
    pub fn to_lower(&mut self) {
        self.data = self.data.to_lowercase();
    }

    /// Remove the last character
    pub fn remove_last_char(&mut self) {
        if !self.data.is_empty() {
            self.data.pop();
        }
    }

    /// Format the string using format! style formatting
    pub fn format(&mut self, args: fmt::Arguments<'_>) {
        self.data = format!("{}", args);
    }

    /// Compare with another AsciiString (case sensitive)
    pub fn compare(&self, other: &AsciiString) -> Ordering {
        self.data.cmp(&other.data)
    }

    /// Compare with a string slice (case sensitive)
    pub fn compare_str(&self, s: &str) -> Ordering {
        self.data.as_str().cmp(s)
    }

    /// Compare with another AsciiString (case insensitive)
    pub fn compare_no_case(&self, other: &AsciiString) -> Ordering {
        self.data.to_lowercase().cmp(&other.data.to_lowercase())
    }

    /// Compare with a string slice (case insensitive)
    pub fn compare_no_case_str(&self, s: &str) -> Ordering {
        self.data.to_lowercase().cmp(&s.to_lowercase())
    }

    /// Find a character in the string
    pub fn find(&self, c: char) -> Option<usize> {
        self.data.find(c)
    }

    /// Find a character from the end of the string
    pub fn reverse_find(&self, c: char) -> Option<usize> {
        self.data.rfind(c)
    }

    /// Check if string starts with the given prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.data.starts_with(prefix)
    }

    /// Check if string starts with the given prefix (case insensitive)
    pub fn starts_with_no_case(&self, prefix: &str) -> bool {
        self.data.to_lowercase().starts_with(&prefix.to_lowercase())
    }

    /// Check if string ends with the given suffix
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.data.ends_with(suffix)
    }

    /// Check if string ends with the given suffix (case insensitive)
    pub fn ends_with_no_case(&self, suffix: &str) -> bool {
        self.data.to_lowercase().ends_with(&suffix.to_lowercase())
    }

    /// Check if the string contains a substring
    pub fn contains(&self, s: &str) -> bool {
        self.data.contains(s)
    }

    /// Extract the next token from the string
    /// This modifies the current string by removing the extracted token
    pub fn next_token(&mut self, seps: Option<&str>) -> Option<AsciiString> {
        if self.is_empty() {
            return None;
        }

        let separators = seps.unwrap_or(" \n\r\t");

        // Create a copy to avoid borrow issues
        let data_copy = self.data.clone();

        // Skip leading separators
        let trimmed = data_copy.trim_start_matches(|c: char| separators.contains(c));

        if trimmed.is_empty() {
            self.clear();
            return None;
        }

        // Find the end of the token
        if let Some(end_pos) = trimmed.find(|c: char| separators.contains(c)) {
            let token = &trimmed[..end_pos];
            let remainder = &trimmed[end_pos..];

            // Skip separators in remainder
            let remainder_trimmed = remainder.trim_start_matches(|c: char| separators.contains(c));

            self.data = remainder_trimmed.to_string();
            Some(AsciiString::from(token))
        } else {
            // Entire remaining string is the token
            let token = AsciiString::from(trimmed);
            self.clear();
            Some(token)
        }
    }

    /// Check if the string is "None" (case insensitive)
    pub fn is_none(&self) -> bool {
        self.data.eq_ignore_ascii_case("none")
    }

    /// Check if the string is not "None" (case insensitive)
    pub fn is_not_none(&self) -> bool {
        !self.is_none()
    }

    /// Get a mutable buffer for reading data (for compatibility with C++ API)
    pub fn get_buffer_for_read(&mut self, len: usize) -> &mut String {
        self.data.clear();
        self.data.reserve(len);
        &mut self.data
    }
}

impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl From<String> for AsciiString {
    fn from(s: String) -> Self {
        Self { data: s }
    }
}

impl From<&str> for AsciiString {
    fn from(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }
}

impl FromStr for AsciiString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(s))
    }
}

impl AsRef<str> for AsciiString {
    fn as_ref(&self) -> &str {
        &self.data
    }
}

impl AsRef<String> for AsciiString {
    fn as_ref(&self) -> &String {
        &self.data
    }
}

impl std::ops::Deref for AsciiString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl AsciiString {
    pub fn as_string(&self) -> &String {
        &self.data
    }

    pub fn as_mut_string(&mut self) -> &mut String {
        &mut self.data
    }
}

impl std::borrow::Borrow<str> for AsciiString {
    fn borrow(&self) -> &str {
        &self.data
    }
}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for AsciiString {}

impl PartialEq<str> for AsciiString {
    fn eq(&self, other: &str) -> bool {
        self.data == other
    }
}

impl PartialEq<&str> for AsciiString {
    fn eq(&self, other: &&str) -> bool {
        self.data == *other
    }
}

impl PartialEq<String> for AsciiString {
    fn eq(&self, other: &String) -> bool {
        &self.data == other
    }
}

impl PartialOrd for AsciiString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AsciiString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }
}

impl Hash for AsciiString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut s = AsciiString::default();
        assert!(s.is_empty());
        assert_eq!(s.get_length(), 0);

        s.set("Hello World");
        assert!(!s.is_empty());
        assert_eq!(s.get_length(), 11);
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn test_concat() {
        let mut s = AsciiString::from("Hello");
        s.concat(" World");
        assert_eq!(s.as_str(), "Hello World");

        let mut s2 = AsciiString::from("!");
        s.concat_ascii_string(&s2);
        assert_eq!(s.as_str(), "Hello World!");
    }

    #[test]
    fn test_case_operations() {
        let mut s = AsciiString::from("Hello World");
        s.to_lower();
        assert_eq!(s.as_str(), "hello world");
    }

    #[test]
    fn test_starts_ends_with() {
        let s = AsciiString::from("Hello World");
        assert!(s.starts_with("Hello"));
        assert!(s.ends_with("World"));
        assert!(s.starts_with_no_case("hello"));
        assert!(s.ends_with_no_case("world"));
    }

    #[test]
    fn test_tokenization() {
        let mut s = AsciiString::from("token1 token2,token3");

        let token1 = s.next_token(Some(" ,")).unwrap();
        assert_eq!(token1.as_str(), "token1");

        let token2 = s.next_token(Some(" ,")).unwrap();
        assert_eq!(token2.as_str(), "token2");

        let token3 = s.next_token(Some(" ,")).unwrap();
        assert_eq!(token3.as_str(), "token3");

        assert!(s.next_token(Some(" ,")).is_none());
    }

    #[test]
    fn test_is_none() {
        let s1 = AsciiString::from("None");
        let s2 = AsciiString::from("NONE");
        let s3 = AsciiString::from("none");
        let s4 = AsciiString::from("Something");

        assert!(s1.is_none());
        assert!(s2.is_none());
        assert!(s3.is_none());
        assert!(!s4.is_none());
        assert!(s4.is_not_none());
    }

    #[test]
    fn test_trim() {
        let mut s = AsciiString::from("  Hello World  ");
        s.trim();
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn test_find() {
        let s = AsciiString::from("Hello World");
        assert_eq!(s.find('o'), Some(4));
        assert_eq!(s.reverse_find('o'), Some(7));
        assert_eq!(s.find('z'), None);
    }
}
