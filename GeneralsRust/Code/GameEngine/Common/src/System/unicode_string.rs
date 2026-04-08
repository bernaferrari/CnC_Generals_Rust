//! Compatibility module for C++ Common/System/UnicodeString.cpp / UnicodeString.h
//! Re-exports the implementation from crate::common::system::unicode_string.
//!
//! PARITY_NOTE: The C++ UnicodeString is a wide-string (UTF-16) class with
//! format methods, case conversion, tokenization, find/replace, trim, and
//! transcoding to/from AsciiString and UTF-8.  The Rust port uses
//! `pub type UnicodeString = String` (native UTF-8) and delegates most
//! operations to std::str/String methods.  Save/load parity is maintained
//! because the Xfer layer reads/writes UTF-8 strings.

pub use crate::common::system::unicode_string::*;
