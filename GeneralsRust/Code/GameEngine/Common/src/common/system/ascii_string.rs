//! ASCII String Utilities
//!
//! Provides ASCII string handling and conversion utilities for the GameEngine.

use std::ffi::CString;

/// ASCII string type for C++ compatibility
pub type AsciiString = String;

/// Convert a Rust string to an ASCII C-compatible string
pub fn to_ascii_cstring(s: &str) -> Result<CString, std::ffi::NulError> {
    CString::new(s)
}

/// Convert a C string to a Rust string
pub fn from_ascii_cstring(cstr: &CString) -> &str {
    cstr.to_str().unwrap_or("")
}

/// Check if a string contains only ASCII characters
pub fn is_ascii(s: &str) -> bool {
    s.is_ascii()
}
