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
//! The reference counting implementation mirrors the C++ AsciiStringData pattern:
//! - AsciiStringData holds the refcount and string data
//! - Copies share the underlying buffer (incrementing refcount via Arc)
//! - Mutations create unique buffers when needed (copy-on-write semantics)
//!
//! Steven Johnson, October 2001
//! Rust conversion: 2025

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::Arc;

/// Maximum length for formatted strings (matches C++ MAX_FORMAT_BUF_LEN)
pub const MAX_FORMAT_BUF_LEN: usize = 2048;

/// Maximum total length of any AsciiString (matches C++ MAX_LEN)
pub const MAX_LEN: usize = 32767;

/// Internal string data with reference counting via Arc.
///
/// This mirrors the C++ AsciiStringData structure:
/// - m_refCount: handled by Arc internally (thread-safe reference counting)
/// - m_numCharsAllocated: stored as String capacity
/// - string data: stored in the inner String
#[derive(Debug, Clone)]
struct AsciiStringData {
    /// The actual string content
    data: String,
}

impl AsciiStringData {
    /// Create new string data with the given content
    fn new(s: String) -> Self {
        Self { data: s }
    }

    /// Get the capacity (matches C++ m_numCharsAllocated)
    fn capacity(&self) -> usize {
        self.data.capacity()
    }
}

/// ASCII String class that provides efficient string operations with reference counting.
///
/// This is the fundamental single-byte string type used in the Generals code base.
/// It uses reference counting (via Arc<AsciiStringData>) to efficiently share string
/// data between copies. Modifications create new buffers (copy-on-write semantics).
///
/// When an AsciiString is copied, the underlying data is shared via Arc reference counting.
/// When the string is mutated, a unique buffer is allocated if the current buffer is shared.
#[derive(Debug, Clone)]
pub struct AsciiString {
    /// Reference-counted string data (None represents empty string for efficiency)
    inner: Option<Arc<AsciiStringData>>,
}

impl Default for AsciiString {
    fn default() -> Self {
        Self::new()
    }
}

impl AsciiString {
    /// Create a new empty AsciiString
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// Get an empty string (compatibility helper matching C++ TheEmptyString)
    #[allow(non_snake_case)]
    pub fn TheEmptyString() -> Self {
        Self::new()
    }

    /// Create an AsciiString from a string slice
    pub fn from(s: &str) -> Self {
        if s.is_empty() {
            Self::new()
        } else {
            Self {
                inner: Some(Arc::new(AsciiStringData::new(s.to_string()))),
            }
        }
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        match &self.inner {
            Some(data) => data.data.len(),
            None => 0,
        }
    }

    /// Get the length of the string in characters (alias for get_length)
    pub fn len(&self) -> usize {
        self.get_length()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        match &self.inner {
            Some(data) => data.data.is_empty(),
            None => true,
        }
    }

    /// Check if the string is not empty
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    /// Clear the string, making it empty (releases reference)
    pub fn clear(&mut self) {
        self.inner = None;
    }

    /// Get the character at the given index
    pub fn get_char_at(&self, index: usize) -> Option<char> {
        match &self.inner {
            Some(data) => data.data.chars().nth(index),
            None => None,
        }
    }

    /// Get the string as a &str
    pub fn as_str(&self) -> &str {
        match &self.inner {
            Some(data) => &data.data,
            None => "",
        }
    }

    /// Get a C-style string pointer (for compatibility)
    pub fn str(&self) -> &str {
        self.as_str()
    }

    /// Set the contents of the string (creates new buffer)
    pub fn set(&mut self, s: &str) {
        if s.is_empty() {
            self.inner = None;
        } else {
            self.inner = Some(Arc::new(AsciiStringData::new(s.to_string())));
        }
    }

    /// Set from another AsciiString (shares reference via Arc clone)
    pub fn set_from_ascii_string(&mut self, other: &AsciiString) {
        // Arc::clone just increments the refcount, sharing the buffer
        self.inner = other.inner.clone();
    }

    /// Ensure we have a unique buffer for mutation (copy-on-write)
    fn ensure_unique(&mut self) {
        if let Some(ref arc) = self.inner {
            // If there are multiple references, we need our own copy
            if Arc::strong_count(arc) > 1 {
                self.inner = Some(Arc::new(AsciiStringData::new(arc.data.clone())));
            }
        }
    }

    /// Ensure we have a unique buffer with at least the specified capacity
    fn ensure_unique_with_capacity(&mut self, capacity: usize) {
        match &self.inner {
            Some(arc) => {
                // If shared or capacity too small, create new buffer
                if Arc::strong_count(arc) > 1 || arc.capacity() < capacity {
                    let mut new_data = String::with_capacity(capacity);
                    new_data.push_str(&arc.data);
                    self.inner = Some(Arc::new(AsciiStringData::new(new_data)));
                }
            }
            None => {
                self.inner = Some(Arc::new(AsciiStringData::new(String::with_capacity(
                    capacity,
                ))));
            }
        }
    }

    /// Concatenate a string slice
    pub fn concat(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }

        let new_len = self.get_length() + s.len();
        self.ensure_unique_with_capacity(new_len);

        if let Some(ref mut arc) = self.inner {
            // We have unique access now due to ensure_unique_with_capacity
            // Use Arc::get_mut to get mutable access
            if let Some(data) = Arc::get_mut(arc) {
                data.data.push_str(s);
            } else {
                // Fallback: create new buffer
                let mut new_string = String::with_capacity(new_len);
                match &self.inner {
                    Some(old_arc) => new_string.push_str(&old_arc.data),
                    None => {}
                }
                new_string.push_str(s);
                self.inner = Some(Arc::new(AsciiStringData::new(new_string)));
            }
        } else {
            self.inner = Some(Arc::new(AsciiStringData::new(s.to_string())));
        }
    }

    /// Concatenate another AsciiString
    pub fn concat_ascii_string(&mut self, other: &AsciiString) {
        self.concat(other.as_str());
    }

    /// Concatenate a single character
    pub fn push(&mut self, c: char) {
        let new_len = self.get_length() + 1;
        self.ensure_unique_with_capacity(new_len);

        if let Some(ref mut arc) = self.inner {
            if let Some(data) = Arc::get_mut(arc) {
                data.data.push(c);
            } else {
                let mut new_string = String::with_capacity(new_len);
                match &self.inner {
                    Some(old_arc) => new_string.push_str(&old_arc.data),
                    None => {}
                }
                new_string.push(c);
                self.inner = Some(Arc::new(AsciiStringData::new(new_string)));
            }
        } else {
            self.inner = Some(Arc::new(AsciiStringData::new(c.to_string())));
        }
    }

    /// Push a string slice
    pub fn push_str(&mut self, s: &str) {
        self.concat(s);
    }

    /// Remove leading and trailing whitespace
    pub fn trim(&mut self) {
        if let Some(ref arc) = self.inner {
            let trimmed = arc.data.trim();
            if trimmed.len() != arc.data.len() {
                // Content changed, need new buffer
                if trimmed.is_empty() {
                    self.inner = None;
                } else {
                    self.inner = Some(Arc::new(AsciiStringData::new(trimmed.to_string())));
                }
            }
        }
    }

    /// Convert to lowercase
    pub fn to_lower(&mut self) {
        if let Some(ref arc) = self.inner {
            let lower = arc.data.to_lowercase();
            if lower != arc.data {
                self.inner = Some(Arc::new(AsciiStringData::new(lower)));
            }
        }
    }

    /// Remove the last character
    pub fn remove_last_char(&mut self) {
        if let Some(ref arc) = self.inner {
            if !arc.data.is_empty() {
                let mut new_string = arc.data.clone();
                new_string.pop();
                if new_string.is_empty() {
                    self.inner = None;
                } else {
                    self.inner = Some(Arc::new(AsciiStringData::new(new_string)));
                }
            }
        }
    }

    /// Format the string using format! style formatting
    pub fn format(&mut self, args: fmt::Arguments<'_>) {
        let formatted = format!("{}", args);
        if formatted.is_empty() {
            self.inner = None;
        } else {
            self.inner = Some(Arc::new(AsciiStringData::new(formatted)));
        }
    }

    /// Compare with another AsciiString (case sensitive)
    pub fn compare(&self, other: &AsciiString) -> Ordering {
        self.as_str().cmp(other.as_str())
    }

    /// Compare with a string slice (case sensitive)
    pub fn compare_str(&self, s: &str) -> Ordering {
        self.as_str().cmp(s)
    }

    /// Compare with another AsciiString (case insensitive)
    pub fn compare_no_case(&self, other: &AsciiString) -> Ordering {
        self.as_str()
            .to_lowercase()
            .cmp(&other.as_str().to_lowercase())
    }

    /// Compare with a string slice (case insensitive)
    pub fn compare_no_case_str(&self, s: &str) -> Ordering {
        self.as_str().to_lowercase().cmp(&s.to_lowercase())
    }

    /// Find a character in the string
    pub fn find(&self, c: char) -> Option<usize> {
        self.as_str().find(c)
    }

    /// Find a character from the end of the string
    pub fn reverse_find(&self, c: char) -> Option<usize> {
        self.as_str().rfind(c)
    }

    /// Check if string starts with the given prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.as_str().starts_with(prefix)
    }

    /// Check if string starts with the given prefix (case insensitive)
    pub fn starts_with_no_case(&self, prefix: &str) -> bool {
        self.as_str()
            .to_lowercase()
            .starts_with(&prefix.to_lowercase())
    }

    /// Check if string ends with the given suffix
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.as_str().ends_with(suffix)
    }

    /// Check if string ends with the given suffix (case insensitive)
    pub fn ends_with_no_case(&self, suffix: &str) -> bool {
        self.as_str()
            .to_lowercase()
            .ends_with(&suffix.to_lowercase())
    }

    /// Check if the string contains a substring
    pub fn contains(&self, s: &str) -> bool {
        self.as_str().contains(s)
    }

    /// Extract the next token from the string
    /// This modifies the current string by removing the extracted token
    pub fn next_token(&mut self, seps: Option<&str>) -> Option<AsciiString> {
        if self.is_empty() {
            return None;
        }

        let separators = seps.unwrap_or(" \n\r\t");

        // Clone the data to avoid borrow issues
        let data = match &self.inner {
            Some(arc) => arc.data.clone(),
            None => return None,
        };

        // Skip leading separators
        let trimmed = data.trim_start_matches(|c: char| separators.contains(c));

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

            self.set(remainder_trimmed);
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
        self.as_str().eq_ignore_ascii_case("none")
    }

    /// Check if the string is not "None" (case insensitive)
    pub fn is_not_none(&self) -> bool {
        !self.is_none()
    }

    /// Get a mutable buffer for reading data (for compatibility with C++ API)
    /// This ensures the buffer is NOT shared.
    pub fn get_buffer_for_read(&mut self, len: usize) -> &mut String {
        // Always create a new unique buffer for reading
        self.inner = Some(Arc::new(AsciiStringData::new(String::with_capacity(len))));

        // Get mutable reference - we know it's unique
        if let Some(ref mut arc) = self.inner {
            if let Some(data) = Arc::get_mut(arc) {
                return &mut data.data;
            }
        }
        // This should never happen since we just created a unique Arc
        unreachable!("get_buffer_for_read should always have unique access");
    }

    /// Get the reference count of the underlying data (for debugging)
    pub fn ref_count(&self) -> usize {
        match &self.inner {
            Some(arc) => Arc::strong_count(arc),
            None => 0,
        }
    }

    /// Check if this string shares data with another (for debugging)
    pub fn shares_data_with(&self, other: &AsciiString) -> bool {
        match (&self.inner, &other.inner) {
            (Some(arc1), Some(arc2)) => Arc::ptr_eq(arc1, arc2),
            _ => false,
        }
    }

    /// Get mutable access to the internal string data.
    /// This ensures the buffer is unique (copy-on-write semantics).
    /// Returns an empty string if the AsciiString is empty.
    pub fn as_mut_string(&mut self) -> &mut str {
        if self.inner.is_none() {
            self.inner = Some(Arc::new(AsciiStringData::new(String::new())));
        }

        if let Some(ref mut arc) = self.inner {
            // If shared, create a unique copy
            if Arc::strong_count(arc) > 1 {
                let data = arc.data.clone();
                self.inner = Some(Arc::new(AsciiStringData::new(data)));
            }

            // Now get mutable access - safe because we ensured uniqueness above
            if let Some(ref mut arc) = self.inner {
                if let Some(data) = Arc::get_mut(arc) {
                    return &mut data.data;
                }
            }
        }

        // Fallback - should never reach here. Arc::get_mut always succeeds after
        // the strong_count check above.
        unreachable!("as_mut_string: should always have unique access after copy-on-write")
    }

    /// Get mutable access to the internal String buffer for xfer operations.
    /// This ensures the buffer is unique (copy-on-write semantics).
    /// Returns a mutable reference to the internal String.
    pub fn as_mut_string_buffer(&mut self) -> &mut String {
        if self.inner.is_none() {
            self.inner = Some(Arc::new(AsciiStringData::new(String::new())));
        }

        if let Some(ref mut arc) = self.inner {
            // If shared, create a unique copy
            if Arc::strong_count(arc) > 1 {
                let data = arc.data.clone();
                self.inner = Some(Arc::new(AsciiStringData::new(data)));
            }

            // Now get mutable access - safe because we ensured uniqueness above
            if let Some(ref mut arc) = self.inner {
                if let Some(data) = Arc::get_mut(arc) {
                    return &mut data.data;
                }
            }
        }

        // Fallback - should never reach here
        unreachable!("as_mut_string_buffer should always have unique access");
    }
}

impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<String> for AsciiString {
    fn from(s: String) -> Self {
        if s.is_empty() {
            Self::new()
        } else {
            Self {
                inner: Some(Arc::new(AsciiStringData::new(s))),
            }
        }
    }
}

impl From<&str> for AsciiString {
    fn from(s: &str) -> Self {
        Self::from(s)
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
        self.as_str()
    }
}

impl std::ops::Deref for AsciiString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for AsciiString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for AsciiString {}

impl PartialEq<str> for AsciiString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for AsciiString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for AsciiString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialOrd for AsciiString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AsciiString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for AsciiString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
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
    fn test_refcount_sharing() {
        let s1 = AsciiString::from("hello");
        let s2 = s1.clone();

        // Both should share the same underlying data
        assert!(s1.shares_data_with(&s2));
        assert_eq!(s1.ref_count(), 2);
        assert_eq!(s2.ref_count(), 2);
        assert_eq!(s1.as_str(), "hello");
        assert_eq!(s2.as_str(), "hello");
    }

    #[test]
    fn test_copy_on_write() {
        let s1 = AsciiString::from("hello");
        let mut s2 = s1.clone();

        // They share data initially
        assert!(s1.shares_data_with(&s2));

        // Modifying s2 should create a new buffer
        s2.concat(" world");

        // They no longer share data
        assert!(!s1.shares_data_with(&s2));
        assert_eq!(s1.as_str(), "hello");
        assert_eq!(s2.as_str(), "hello world");
    }

    #[test]
    fn test_set_from_ascii_string() {
        let s1 = AsciiString::from("shared data");
        let mut s2 = AsciiString::new();

        s2.set_from_ascii_string(&s1);

        // They should share data
        assert!(s1.shares_data_with(&s2));
        assert_eq!(s1.ref_count(), 2);
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

    #[test]
    fn test_empty_string_efficiency() {
        let s = AsciiString::new();
        assert!(s.is_empty());
        assert_eq!(s.ref_count(), 0); // No Arc allocated for empty string
    }

    #[test]
    fn test_clear_releases_reference() {
        let mut s = AsciiString::from("hello");
        assert_eq!(s.ref_count(), 1);

        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.ref_count(), 0);
    }
}
