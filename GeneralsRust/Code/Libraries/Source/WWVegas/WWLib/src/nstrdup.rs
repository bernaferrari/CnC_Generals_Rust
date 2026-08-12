//! String duplication utility mirroring WWLib `nstrdup`.
//!
//! This module provides a faithful Rust implementation of the `nstrdup` function
//! from WWLib, which duplicates a C-style string using heap allocation.
//!
//! # C++ Source
//! Original implementation in `GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/nstrdup.cpp`
//!
//! ```cpp
//! char * nstrdup(const char *str)
//! {
//!     if(str == 0) return 0;
//!     char *retval = W3DNEWARRAY char [strlen(str) + 1];
//!     strcpy(retval, str);
//!     return retval;
//! }
//! ```

use std::ffi::CString;

/// Duplicates a string slice, returning an owned `String`.
///
/// This is the safe Rust equivalent of `nstrdup`. Returns `None` if the input
/// is `None` (mirroring the null check in C++), otherwise returns a heap-allocated
/// copy of the string.
///
/// # Example
/// ```rust
/// use wwlib_rust::nstrdup::nstrdup;
///
/// let original = "Hello, World!";
/// let duplicate = nstrdup(Some(original));
/// assert_eq!(duplicate, Some("Hello, World!".to_string()));
///
/// let null_result: Option<String> = nstrdup(None);
/// assert_eq!(null_result, None);
/// ```
pub fn nstrdup(s: Option<&str>) -> Option<String> {
    s.map(|s| s.to_string())
}

/// Duplicates a byte slice as a CString.
///
/// Returns `None` if the input is `None` or if the bytes contain an interior
/// null byte (which would make an invalid CString).
pub fn nstrdup_bytes(bytes: Option<&[u8]>) -> Option<CString> {
    bytes.and_then(|b| CString::new(b).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nstrdup_some() {
        let result = nstrdup(Some("hello"));
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_nstrdup_none() {
        let result: Option<String> = nstrdup(None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_nstrdup_empty_string() {
        let result = nstrdup(Some(""));
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_nstrdup_unicode() {
        let result = nstrdup(Some("Hello \u{1F600}"));
        assert_eq!(result, Some("Hello \u{1F600}".to_string()));
    }

    #[test]
    fn test_nstrdup_independence() {
        let original = String::from("test");
        let duplicate = nstrdup(Some(&original)).unwrap();
        drop(original);
        assert_eq!(duplicate, "test");
    }

    #[test]
    fn test_nstrdup_bytes_valid() {
        let result = nstrdup_bytes(Some(b"hello"));
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_bytes(), b"hello");
    }

    #[test]
    fn test_nstrdup_bytes_none() {
        let result = nstrdup_bytes(None);
        assert!(result.is_none());
    }

    #[test]
    fn test_nstrdup_bytes_interior_nul() {
        let result = nstrdup_bytes(Some(b"hel\0lo"));
        assert!(result.is_none());
    }

    #[test]
    fn test_nstrdup_bytes_empty() {
        let result = nstrdup_bytes(Some(b""));
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_bytes(), b"");
    }
}
