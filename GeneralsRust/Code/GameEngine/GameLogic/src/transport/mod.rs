// Transport module - UDP networking layer for C&C Generals
//
// This module provides byte-for-byte compatibility with the C++ UDP transport implementation.
// It handles packet framing, encryption, CRC validation, and reliable delivery.

pub mod crc;
pub mod encryption;
pub mod packet;
pub mod udp;

pub use crc::Crc;
pub use encryption::{decrypt_buffer, encrypt_buffer};
pub use packet::{TransportMessage, TransportMessageHeader, MAX_MESSAGE_LEN, MAX_PACKET_SIZE};
pub use udp::UdpTransport;

/// Magic number for identifying Generals packets (0xF00D)
pub const GENERALS_MAGIC_NUMBER: u16 = 0xF00D;

/// Maximum number of messages in transport buffers
pub const MAX_MESSAGES: usize = 128;

/// Retry timeout in milliseconds
pub const RETRY_TIME_MS: u64 = 2000;

/// Keep-alive interval in seconds
pub const KEEPALIVE_INTERVAL_SECS: u64 = 20;

#[cfg(test)]
mod tests;
