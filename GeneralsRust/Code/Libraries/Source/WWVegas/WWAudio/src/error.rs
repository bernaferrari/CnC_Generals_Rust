//! Error types and utilities for the WPAudio system.

use thiserror::Error;

/// WPAudio result type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for WPAudio operations
#[derive(Error, Debug)]
pub enum Error {
    /// Device-related errors
    #[error("Audio device error: {0}")]
    Device(#[from] DeviceError),

    /// Channel-related errors
    #[error("Audio channel error: {0}")]
    Channel(#[from] ChannelError),

    /// Source/format errors
    #[error("Audio source error: {0}")]
    Source(#[from] SourceError),

    /// Memory allocation errors
    #[error("Memory error: {0}")]
    Memory(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Streaming-related errors
    #[error("Streaming error: {0}")]
    Stream(#[from] StreamError),

    /// Generic audio error
    #[error("Audio error: {0}")]
    Audio(String),
}

/// Device-specific errors
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device not found")]
    NotFound,
    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Device access denied")]
    AccessDenied,
    #[error("Unsupported format")]
    UnsupportedFormat,
    #[error("Device not initialized")]
    NotInitialized,
}

/// Channel-specific errors  
#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Channel allocation failed")]
    AllocationFailed,
    #[error("Channel not available")]
    NotAvailable,
    #[error("Invalid channel state: {0}")]
    InvalidState(String),
}

/// Source-specific errors
#[derive(Error, Debug)]
pub enum SourceError {
    #[error("Source not found: {0}")]
    NotFound(String),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error("Compression error: {0}")]
    CompressionError(String),
}

/// Streaming-specific errors
#[derive(Error, Debug)]
pub enum StreamError {
    #[error("Stream buffer overflow")]
    BufferOverflow,
    #[error("Stream buffer underrun")]
    BufferUnderrun,
    #[error("Stream not initialized")]
    NotInitialized,
    #[error("Stream access conflict: {0}")]
    AccessConflict(String),
    #[error("Stream format mismatch: {0}")]
    FormatMismatch(String),
    #[error("Stream synchronization error")]
    SyncError,
    #[error("Stream EOF reached")]
    EndOfStream,
    #[error("Stream operation failed: {0}")]
    OperationFailed(String),
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorKind {
    /// Recoverable errors that may be retried
    Recoverable,
    /// Fatal errors requiring system restart
    Fatal,
    /// Configuration errors
    Configuration,
    /// Resource exhaustion
    ResourceExhaustion,
}
