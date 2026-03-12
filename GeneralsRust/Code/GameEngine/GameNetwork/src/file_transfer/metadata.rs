//! File metadata structures

use serde::{Deserialize, Serialize};

/// File metadata for transfer operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Filename
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
    /// File checksum (SHA-256 bytes)
    pub checksum: [u8; 32],
    /// Transfer type
    pub transfer_type: TransferType,
}

/// Transfer type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum TransferType {
    /// Generic file transfer
    Generic = 0,
    /// Map file
    Map = 1,
    /// Replay file
    Replay = 2,
    /// Mod file
    Mod = 3,
    /// Savegame file
    SaveGame = 4,
    /// Asset/big file or misc asset bundle
    Asset = 5,
}
