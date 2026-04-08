//! Compatibility module for C++ Common/MiniLog.cpp / MiniLog.h
//! Re-exports the implementation from crate::common::mini_log.
//!
//! PARITY_NOTE: The C++ MiniLog is a small logging utility that writes
//! timestamped messages to a file, used primarily during map loading and
//! editor operations.  The Rust port preserves the MiniLog struct with
//! open/close/log methods but uses Rust's std::io for file I/O.

pub use crate::common::mini_log::*;
