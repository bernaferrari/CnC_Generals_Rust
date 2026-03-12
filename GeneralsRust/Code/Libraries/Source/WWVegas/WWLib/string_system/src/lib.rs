//! String system for Command & Conquer Generals Zero Hour
//!
//! This crate provides efficient string handling with reference-counted,
//! copy-on-write semantics similar to the original C++ implementation.
//!
//! The system includes:
//! - AsciiString: Single-byte string with ref-counting and COW
//! - UnicodeString: Wide character string with similar functionality
//! - String operations (concat, trim, format, search, etc.)
//! - Memory-efficient storage using the memory pool system

use base_types::*;
use std::alloc::{alloc, dealloc, Layout};
use std::cmp::Ordering;
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicU16, Ordering as AtomicOrdering};

/// Maximum length of any string in characters
pub const MAX_STRING_LEN: usize = 32767;

/// Maximum length of format buffer
pub const MAX_FORMAT_BUF_LEN: usize = 2048;

/// Reference-counted string data for AsciiString
#[derive(Debug)]
struct AsciiStringData {
    /// Reference count
    ref_count: AtomicU16,

    /// Number of characters allocated (including null terminator)
    num_chars_allocated: u16,

    /// Debug pointer for easier debugging
    #[cfg(feature = "debug")]
    debug_ptr: *const u8,
    // The actual string data follows this structure
    // We use a flexible array member pattern in Rust
}

impl AsciiStringData {
    /// Create a new AsciiStringData with the given capacity
    fn new(capacity: usize) -> Result<*mut Self, String> {
        if capacity > MAX_STRING_LEN {
            return Err(format!(
                "String capacity {} exceeds maximum {}",
                capacity, MAX_STRING_LEN
            ));
        }

        let total_size = std::mem::size_of::<Self>() + capacity;
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<Self>())
            .map_err(|e| format!("Failed to create layout: {}", e))?;

        let ptr = unsafe { alloc(layout) as *mut Self };
        if ptr.is_null() {
            return Err("Failed to allocate memory for AsciiStringData".to_string());
        }

        unsafe {
            (*ptr).ref_count = AtomicU16::new(1);
            (*ptr).num_chars_allocated = capacity as u16;

            #[cfg(feature = "debug")]
            {
                (*ptr).debug_ptr = Self::data_ptr(ptr);
            }
        }

        Ok(ptr)
    }

    /// Get a pointer to the string data
    fn data_ptr(data: *mut Self) -> *mut u8 {
        unsafe { (data as *mut u8).add(std::mem::size_of::<Self>()) }
    }

    /// Get a const pointer to the string data
    fn data_ptr_const(data: *const Self) -> *const u8 {
        unsafe { (data as *const u8).add(std::mem::size_of::<Self>()) }
    }

    /// Get the string as a &str (unsafe, caller must ensure validity)
    unsafe fn as_str<'a>(data: *const Self) -> &'a str {
        let data_ptr = Self::data_ptr_const(data);
        let len = libc::strlen(data_ptr as *const libc::c_char);
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data_ptr, len))
    }

    /// Set the string data from a &str
    unsafe fn set_from_str(data: *mut Self, s: &str) {
        let data_ptr = Self::data_ptr(data);
        let capacity = (*data).num_chars_allocated as usize;

        if s.len() >= capacity {
            // String is too long, truncate
            ptr::copy_nonoverlapping(s.as_ptr(), data_ptr, capacity - 1);
            *data_ptr.add(capacity - 1) = 0;
        } else {
            ptr::copy_nonoverlapping(s.as_ptr(), data_ptr, s.len());
            *data_ptr.add(s.len()) = 0;
        }
    }

    /// Increment reference count
    fn increment_ref_count(data: *mut Self) {
        unsafe {
            (*data).ref_count.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    /// Decrement reference count and return true if it reaches zero
    fn decrement_ref_count(data: *mut Self) -> bool {
        unsafe { (*data).ref_count.fetch_sub(1, AtomicOrdering::Relaxed) == 1 }
    }

    /// Get current reference count
    fn get_ref_count(data: *const Self) -> u16 {
        unsafe { (*data).ref_count.load(AtomicOrdering::Relaxed) }
    }

    /// Free the string data
    unsafe fn free(data: *mut Self) {
        let total_size = std::mem::size_of::<Self>() + (*data).num_chars_allocated as usize;
        let layout = Layout::from_size_align_unchecked(total_size, std::mem::align_of::<Self>());
        dealloc(data as *mut u8, layout);
    }
}

/// AsciiString - Reference-counted ASCII string with copy-on-write semantics
pub struct AsciiString {
    /// Pointer to the reference-counted string data
    data: *mut AsciiStringData,
}

impl Clone for AsciiString {
    fn clone(&self) -> Self {
        if !self.data.is_null() {
            AsciiStringData::increment_ref_count(self.data);
        }
        Self { data: self.data }
    }
}

impl AsciiString {
    /// The empty string constant
    pub const THE_EMPTY_STRING: AsciiString = AsciiString {
        data: ptr::null_mut(),
    };

    /// Create a new empty AsciiString
    pub fn new() -> Self {
        Self {
            data: ptr::null_mut(),
        }
    }

    /// Create an AsciiString from a &str
    pub fn from_str(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Ok(Self::THE_EMPTY_STRING);
        }

        let capacity = s.len() + 1; // +1 for null terminator
        let data = AsciiStringData::new(capacity)?;

        unsafe {
            AsciiStringData::set_from_str(data, s);
        }

        Ok(Self { data })
    }

    /// Create an AsciiString from a C-style string pointer
    pub fn from_c_str(s: *const libc::c_char) -> Result<Self, String> {
        if s.is_null() {
            return Ok(Self::THE_EMPTY_STRING);
        }

        unsafe {
            let len = libc::strlen(s);
            let slice = std::slice::from_raw_parts(s as *const u8, len);
            let rust_str =
                std::str::from_utf8(slice).map_err(|e| format!("Invalid UTF-8 sequence: {}", e))?;
            Self::from_str(rust_str)
        }
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        if self.data.is_null() {
            0
        } else {
            unsafe { AsciiStringData::as_str(self.data).len() }
        }
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> Bool {
        self.get_length() == 0
    }

    /// Clear the string (make it empty)
    pub fn clear(&mut self) {
        if !self.data.is_null() {
            unsafe {
                if AsciiStringData::decrement_ref_count(self.data) {
                    AsciiStringData::free(self.data);
                }
            }
            self.data = ptr::null_mut();
        }
    }

    /// Get a const pointer to the string data
    pub fn str(&self) -> *const libc::c_char {
        if self.data.is_null() {
            b"\0".as_ptr() as *const libc::c_char
        } else {
            AsciiStringData::data_ptr_const(self.data) as *const libc::c_char
        }
    }

    /// Get the string as a &str
    pub fn as_str(&self) -> &str {
        if self.data.is_null() {
            ""
        } else {
            unsafe { AsciiStringData::as_str(self.data) }
        }
    }

    /// Get the character at the specified index
    pub fn get_char_at(&self, index: usize) -> Option<char> {
        let s = self.as_str();
        s.chars().nth(index)
    }

    /// Set the string from another AsciiString
    pub fn set(&mut self, other: &AsciiString) {
        if !self.data.is_null() {
            unsafe {
                if AsciiStringData::decrement_ref_count(self.data) {
                    AsciiStringData::free(self.data);
                }
            }
        }

        self.data = other.data;
        if !self.data.is_null() {
            AsciiStringData::increment_ref_count(self.data);
        }
    }

    /// Set the string from a &str
    pub fn set_str(&mut self, s: &str) -> Result<(), String> {
        if s.is_empty() {
            self.clear();
            return Ok(());
        }

        let capacity = s.len() + 1;

        // Check if we can reuse the existing buffer
        if !self.data.is_null() {
            unsafe {
                let ref_count = AsciiStringData::get_ref_count(self.data);
                let allocated = (*self.data).num_chars_allocated as usize;

                if ref_count == 1 && allocated >= capacity {
                    // Reuse existing buffer
                    AsciiStringData::set_from_str(self.data, s);
                    return Ok(());
                }
            }
        }

        // Need to allocate new buffer
        if !self.data.is_null() {
            unsafe {
                if AsciiStringData::decrement_ref_count(self.data) {
                    AsciiStringData::free(self.data);
                }
            }
        }

        let data = AsciiStringData::new(capacity)?;
        unsafe {
            AsciiStringData::set_from_str(data, s);
        }
        self.data = data;

        Ok(())
    }

    /// Concatenate another AsciiString to this one
    pub fn concat(&mut self, other: &AsciiString) -> Result<(), String> {
        if other.is_empty() {
            return Ok(());
        }

        let other_str = other.as_str();
        self.concat_str(other_str)
    }

    /// Concatenate a &str to this string
    pub fn concat_str(&mut self, s: &str) -> Result<(), String> {
        if s.is_empty() {
            return Ok(());
        }

        let current_str = self.as_str();
        let new_str = format!("{}{}", current_str, s);
        self.set_str(&new_str)
    }

    /// Concatenate a single character to this string
    pub fn concat_char(&mut self, c: char) -> Result<(), String> {
        let new_str = format!("{}{}", self.as_str(), c);
        self.set_str(&new_str)
    }

    /// Remove leading and trailing whitespace
    pub fn trim(&mut self) -> Result<(), String> {
        let trimmed = self.as_str().trim().to_string();
        self.set_str(&trimmed)
    }

    /// Convert the string to lowercase
    pub fn to_lower(&mut self) -> Result<(), String> {
        let lower = self.as_str().to_lowercase();
        self.set_str(&lower)
    }

    /// Remove the last character from the string
    pub fn remove_last_char(&mut self) {
        let s = self.as_str().to_string();
        if !s.is_empty() {
            let new_len = s.len() - 1;
            let truncated = &s[..new_len];
            let _ = self.set_str(truncated); // Ignore error for now
        }
    }

    /// Format the string using sprintf-style formatting
    pub fn format(format_str: &str, args: &[&dyn fmt::Display]) -> Result<Self, String> {
        let mut formatted = String::new();

        // Simple format implementation (could be enhanced)
        let mut arg_iter = args.iter();
        let mut chars = format_str.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(next_ch) = chars.peek() {
                    match next_ch {
                        '%' => {
                            formatted.push('%');
                            chars.next();
                        }
                        's' | 'd' | 'f' => {
                            if let Some(arg) = arg_iter.next() {
                                formatted.push_str(&format!("{}", arg));
                            } else {
                                return Err("Not enough arguments for format string".to_string());
                            }
                            chars.next();
                        }
                        _ => {
                            formatted.push(ch);
                        }
                    }
                } else {
                    formatted.push(ch);
                }
            } else {
                formatted.push(ch);
            }
        }

        if formatted.len() > MAX_FORMAT_BUF_LEN {
            return Err(format!(
                "Formatted string too long: {} > {}",
                formatted.len(),
                MAX_FORMAT_BUF_LEN
            ));
        }

        Self::from_str(&formatted)
    }

    /// Find the first occurrence of a substring
    pub fn find(&self, substring: &str) -> Option<usize> {
        self.as_str().find(substring)
    }

    /// Find the last occurrence of a substring
    pub fn reverse_find(&self, substring: &str) -> Option<usize> {
        self.as_str().rfind(substring)
    }

    /// Extract a substring
    pub fn substr(&self, start: usize, length: usize) -> Result<Self, String> {
        let s = self.as_str();
        if start >= s.len() {
            return Ok(Self::THE_EMPTY_STRING);
        }

        let end = (start + length).min(s.len());
        let substr = &s[start..end];
        Self::from_str(substr)
    }

    /// Compare two strings
    pub fn compare(&self, other: &AsciiString) -> Ordering {
        self.as_str().cmp(other.as_str())
    }

    /// Compare with a &str
    pub fn compare_str(&self, other: &str) -> Ordering {
        self.as_str().cmp(other)
    }

    /// Check if the string starts with a prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.as_str().starts_with(prefix)
    }

    /// Check if the string ends with a suffix
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.as_str().ends_with(suffix)
    }

    /// Replace all occurrences of a substring
    pub fn replace(&mut self, from: &str, to: &str) -> Result<(), String> {
        let new_str = self.as_str().replace(from, to);
        self.set_str(&new_str)
    }

    /// Convert to UnicodeString
    #[cfg(feature = "unicode")]
    pub fn to_unicode(&self) -> UnicodeString {
        UnicodeString::from_ascii(self)
    }
}

impl Default for AsciiString {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AsciiString {
    fn drop(&mut self) {
        if !self.data.is_null() {
            unsafe {
                if AsciiStringData::decrement_ref_count(self.data) {
                    AsciiStringData::free(self.data);
                }
            }
        }
    }
}

impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AsciiString(\"{}\")", self.as_str())
    }
}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}

impl Eq for AsciiString {}

impl PartialOrd for AsciiString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

impl Ord for AsciiString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl From<&str> for AsciiString {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::THE_EMPTY_STRING)
    }
}

impl From<String> for AsciiString {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or(Self::THE_EMPTY_STRING)
    }
}

impl std::ops::Add for AsciiString {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut result = self;
        let _ = result.concat(&other); // Ignore error for operator
        result
    }
}

impl std::ops::Add<&str> for AsciiString {
    type Output = Self;

    fn add(self, other: &str) -> Self {
        let mut result = self;
        let _ = result.concat_str(other); // Ignore error for operator
        result
    }
}

impl std::ops::AddAssign<&AsciiString> for AsciiString {
    fn add_assign(&mut self, other: &Self) {
        let _ = self.concat(other); // Ignore error for operator
    }
}

impl std::ops::AddAssign<&str> for AsciiString {
    fn add_assign(&mut self, other: &str) {
        let _ = self.concat_str(other); // Ignore error for operator
    }
}

#[cfg(feature = "unicode")]
/// UnicodeString - Reference-counted Unicode string with copy-on-write semantics
#[derive(Clone)]
pub struct UnicodeString {
    /// Pointer to the reference-counted string data
    data: *mut UnicodeStringData,
}

#[cfg(feature = "unicode")]
/// Reference-counted string data for UnicodeString
#[derive(Debug)]
struct UnicodeStringData {
    /// Reference count
    ref_count: AtomicU16,

    /// Number of characters allocated (including null terminator)
    num_chars_allocated: u16,

    /// Debug pointer for easier debugging
    #[cfg(feature = "debug")]
    debug_ptr: *const u16,
    // The actual string data follows this structure
}

#[cfg(feature = "unicode")]
impl UnicodeString {
    /// The empty string constant
    pub const THE_EMPTY_STRING: UnicodeString = UnicodeString {
        data: ptr::null_mut(),
    };

    /// Create a new empty UnicodeString
    pub fn new() -> Self {
        Self {
            data: ptr::null_mut(),
        }
    }

    /// Create from an AsciiString
    pub fn from_ascii(ascii: &AsciiString) -> Self {
        let s = ascii.as_str();
        Self::from_str(s).unwrap_or(Self::THE_EMPTY_STRING)
    }

    /// Create a UnicodeString from a &str
    pub fn from_str(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Ok(Self::THE_EMPTY_STRING);
        }

        // Convert to UTF-16
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let capacity = utf16.len() + 1; // +1 for null terminator

        if capacity > MAX_STRING_LEN {
            return Err(format!(
                "String capacity {} exceeds maximum {}",
                capacity, MAX_STRING_LEN
            ));
        }

        let total_size =
            std::mem::size_of::<UnicodeStringData>() + capacity * std::mem::size_of::<u16>();
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<UnicodeStringData>())
            .map_err(|e| format!("Failed to create layout: {}", e))?;

        let ptr = unsafe { alloc(layout) as *mut UnicodeStringData };
        if ptr.is_null() {
            return Err("Failed to allocate memory for UnicodeStringData".to_string());
        }

        unsafe {
            (*ptr).ref_count = AtomicU16::new(1);
            (*ptr).num_chars_allocated = capacity as u16;

            #[cfg(feature = "debug")]
            {
                (*ptr).debug_ptr = UnicodeStringData::data_ptr(ptr);
            }

            // Copy UTF-16 data
            let data_ptr = UnicodeStringData::data_ptr(ptr);
            ptr::copy_nonoverlapping(utf16.as_ptr(), data_ptr, utf16.len());
            *data_ptr.add(utf16.len()) = 0; // Null terminator
        }

        Ok(Self { data: ptr })
    }

    /// Set the string contents from a &str.
    pub fn set_str(&mut self, s: &str) -> Result<(), String> {
        if s.is_empty() {
            self.clear();
            return Ok(());
        }

        let mut new_value = UnicodeString::from_str(s)?;
        std::mem::swap(self, &mut new_value);
        Ok(())
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        if self.data.is_null() {
            0
        } else {
            unsafe {
                let data_ptr = UnicodeStringData::data_ptr_const(self.data);
                let mut len = 0;
                while *data_ptr.add(len) != 0 {
                    len += 1;
                }
                len
            }
        }
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> Bool {
        self.get_length() == 0
    }

    /// Clear the string
    pub fn clear(&mut self) {
        if !self.data.is_null() {
            unsafe {
                if UnicodeStringData::decrement_ref_count(self.data) {
                    UnicodeStringData::free(self.data);
                }
            }
            self.data = ptr::null_mut();
        }
    }

    /// Get as UTF-8 string
    pub fn as_str(&self) -> &str {
        if self.data.is_null() {
            ""
        } else {
            unsafe {
                let data_ptr = UnicodeStringData::data_ptr_const(self.data);
                let len = self.get_length();
                let utf16_slice = std::slice::from_raw_parts(data_ptr, len);
                let utf8: String = String::from_utf16_lossy(utf16_slice);
                // This is not ideal - we should return a Cow or similar
                // For now, we'll leak the string to return a static reference
                Box::leak(utf8.into_boxed_str())
            }
        }
    }

    /// Convert to AsciiString (lossy conversion)
    pub fn to_ascii(&self) -> AsciiString {
        let utf8 = self.as_str();
        AsciiString::from_str(utf8).unwrap_or(AsciiString::THE_EMPTY_STRING)
    }
}

#[cfg(feature = "unicode")]
impl UnicodeStringData {
    /// Get a pointer to the string data
    fn data_ptr(data: *mut Self) -> *mut u16 {
        unsafe { (data as *mut u8).add(std::mem::size_of::<Self>()) as *mut u16 }
    }

    /// Get a const pointer to the string data
    fn data_ptr_const(data: *const Self) -> *const u16 {
        unsafe { (data as *const u8).add(std::mem::size_of::<Self>()) as *const u16 }
    }

    /// Increment reference count
    fn increment_ref_count(data: *mut Self) {
        unsafe {
            (*data).ref_count.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    /// Decrement reference count and return true if it reaches zero
    fn decrement_ref_count(data: *mut Self) -> bool {
        unsafe { (*data).ref_count.fetch_sub(1, AtomicOrdering::Relaxed) == 1 }
    }

    /// Free the string data
    unsafe fn free(data: *mut Self) {
        let total_size = std::mem::size_of::<Self>()
            + ((*data).num_chars_allocated as usize) * std::mem::size_of::<u16>();
        let layout = Layout::from_size_align_unchecked(total_size, std::mem::align_of::<Self>());
        dealloc(data as *mut u8, layout);
    }
}

#[cfg(feature = "unicode")]
impl Drop for UnicodeString {
    fn drop(&mut self) {
        if !self.data.is_null() {
            unsafe {
                if UnicodeStringData::decrement_ref_count(self.data) {
                    UnicodeStringData::free(self.data);
                }
            }
        }
    }
}

#[cfg(feature = "unicode")]
impl fmt::Display for UnicodeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(feature = "unicode")]
impl fmt::Debug for UnicodeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnicodeString(\"{}\")", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_string_creation() {
        let empty = AsciiString::new();
        assert!(empty.is_empty());
        assert_eq!(empty.get_length(), 0);

        let s = AsciiString::from_str("Hello World").unwrap();
        assert!(!s.is_empty());
        assert_eq!(s.get_length(), 11);
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn test_ascii_string_copy_on_write() {
        let s1 = AsciiString::from_str("Original").unwrap();
        let s2 = s1.clone();

        // Both should point to the same data initially
        assert_eq!(s1.data, s2.data);

        // Modifying s2 should create a new buffer
        let mut s3 = s2;
        let _ = s3.set_str("Modified");

        // s1 and s3 should have different data pointers
        assert_ne!(s1.data, s3.data);
        assert_eq!(s1.as_str(), "Original");
        assert_eq!(s3.as_str(), "Modified");
    }

    #[test]
    fn test_ascii_string_concatenation() {
        let mut s = AsciiString::from_str("Hello").unwrap();
        let _ = s.concat_str(" World");
        assert_eq!(s.as_str(), "Hello World");
        assert_eq!(s.get_length(), 11);
    }

    #[test]
    fn test_ascii_string_trim() {
        let mut s = AsciiString::from_str("  Hello World  ").unwrap();
        let _ = s.trim();
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn test_ascii_string_case_conversion() {
        let mut s = AsciiString::from_str("Hello World").unwrap();
        let _ = s.to_lower();
        assert_eq!(s.as_str(), "hello world");
    }

    #[test]
    fn test_ascii_string_substring() {
        let s = AsciiString::from_str("Hello World").unwrap();
        let substr = s.substr(6, 5).unwrap();
        assert_eq!(substr.as_str(), "World");
    }

    #[test]
    fn test_ascii_string_find() {
        let s = AsciiString::from_str("Hello World").unwrap();
        assert_eq!(s.find("World"), Some(6));
        assert_eq!(s.find("Universe"), None);
    }

    #[test]
    fn test_ascii_string_comparison() {
        let s1 = AsciiString::from_str("Apple").unwrap();
        let s2 = AsciiString::from_str("Banana").unwrap();
        let s3 = AsciiString::from_str("Apple").unwrap();

        assert!(s1 < s2);
        assert_eq!(s1, s3);
        assert!(s1.compare(&s2) == Ordering::Less);
    }

    #[test]
    fn test_ascii_string_format() {
        let formatted = AsciiString::format("Hello %s!", &[&"World"]).unwrap();
        assert_eq!(formatted.as_str(), "Hello World!");
    }

    #[test]
    fn test_ascii_string_operators() {
        let s1 = AsciiString::from_str("Hello").unwrap();
        let s2 = AsciiString::from_str(" World").unwrap();

        let combined = s1 + s2;
        assert_eq!(combined.as_str(), "Hello World");

        let mut s3 = AsciiString::from_str("Hello").unwrap();
        s3 += " World";
        assert_eq!(s3.as_str(), "Hello World");
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn test_unicode_string_creation() {
        let empty = UnicodeString::new();
        assert!(empty.is_empty());

        let s = UnicodeString::from_str("Hello World").unwrap();
        assert!(!s.is_empty());
        assert_eq!(s.get_length(), 11);
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn test_ascii_to_unicode_conversion() {
        let ascii = AsciiString::from_str("Hello World").unwrap();
        let unicode = ascii.to_unicode();
        let back_to_ascii = unicode.to_ascii();

        assert_eq!(ascii.as_str(), back_to_ascii.as_str());
    }
}
