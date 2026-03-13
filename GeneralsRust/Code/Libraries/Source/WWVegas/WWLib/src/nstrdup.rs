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

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Duplicates a C-style string, returning a newly allocated `CString`.
///
/// Mirrors the C++ `nstrdup(const char *str)` function.
/// Returns `None` if the input pointer is null.
///
/// # Safety
/// The input pointer must be either null or point to a valid, null-terminated
/// C string with a lifetime that is valid for the duration of this call.
pub unsafe fn nstrdup_raw(str_ptr: *const c_char) -> *mut c_char {
    if str_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let cstr = unsafe { CStr::from_ptr(str_ptr) };
    let owned = CString::new(cstr.to_bytes()).unwrap_or_else(|_| CString::new("").unwrap());
    owned.into_raw()
}

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

/// Frees a string allocated by `nstrdup_raw`.
///
/// # Safety
/// The pointer must have been returned by `nstrdup_raw` and not yet freed.
/// After this call, the pointer is invalid and must not be used.
pub unsafe fn nstrdup_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

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
    fn test_nstrdup_raw_null() {
        let result = unsafe { nstrdup_raw(std::ptr::null()) };
        assert!(result.is_null());
    }

    #[test]
    fn test_nstrdup_raw_valid() {
        let cstr = CString::new("hello world").unwrap();
        let result = unsafe { nstrdup_raw(cstr.as_ptr()) };
        assert!(!result.is_null());

        let dup_cstr = unsafe { CStr::from_ptr(result) };
        assert_eq!(dup_cstr.to_str().unwrap(), "hello world");

        unsafe { nstrdup_free(result) };
    }

    #[test]
    fn test_nstrdup_raw_free_null() {
        // Should be a no-op
        unsafe { nstrdup_free(std::ptr::null_mut()) };
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
