//! Compatibility adapter for the legacy `GameClient/HotKey.cpp` port.
//!
//! The actual implementation lives in `message_stream::hot_key`; this module
//! keeps the expected file-path parity without duplicating behavior.

pub use crate::message_stream::hot_key::*;
