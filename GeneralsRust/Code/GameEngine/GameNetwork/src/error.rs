//! Error handling for the GameNetwork module
//!
//! Provides comprehensive error types covering all networking scenarios
//! including transport failures, protocol errors, security violations,
//! and game-specific network errors.

use std::io;
use thiserror::Error;
use tokio::time::error::Elapsed;
use tungstenite::Error as TungsteniteError;

/// Result type alias for network operations
pub type NetworkResult<T> = Result<T, NetworkError>;

/// Comprehensive network error types
#[derive(Error, Debug)]
pub enum NetworkError {
    /// Input/Output errors from underlying network operations
    #[error("Network I/O error: {0}")]
    Io(#[from] io::Error),

    /// Timeout errors for network operations
    #[error("Network operation timed out: {0}")]
    Timeout(#[from] Elapsed),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// JSON serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// WebSocket protocol errors
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] TungsteniteError),

    /// QUIC protocol errors
    #[error("QUIC error: {0}")]
    Quic(#[from] quinn::ConnectionError),

    /// TLS/Security errors
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    /// Connection-specific errors
    #[error("Connection error: {message}")]
    Connection { message: String },

    /// Protocol version mismatch
    #[error("Protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: u32, actual: u32 },

    /// Invalid packet format
    #[error("Invalid packet: {reason}")]
    InvalidPacket { reason: String },

    /// Player-related errors
    #[error("Player error: {message}")]
    Player { message: String },

    /// Game command validation errors
    #[error("Invalid command: {reason}")]
    InvalidCommand { reason: String },

    /// Frame synchronization errors
    #[error("Frame sync error: {message}")]
    FrameSync { message: String },

    /// File transfer errors
    #[error("File transfer error: {message}")]
    FileTransfer { message: String },

    /// Authentication and security errors
    #[error("Security error: {message}")]
    Security { message: String },

    /// Anti-cheat violations
    #[error("Anti-cheat violation: {violation}")]
    AntiCheat { violation: String },

    /// Transport layer error
    #[error("Transport error: {message}")]
    Transport { message: String },

    /// Matchmaking service errors
    #[error("Matchmaking error: {message}")]
    Matchmaking { message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Resource exhaustion (too many connections, memory, etc.)
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {limit}")]
    RateLimited { limit: String },

    /// Generic network errors with context
    #[error("Network error: {message}")]
    Generic { message: String },

    /// NAT traversal errors
    #[error("NAT traversal error: {message}")]
    Nat { message: String },
}

impl NetworkError {
    /// Create a new connection error
    pub fn connection<S: Into<String>>(message: S) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Create a new player error
    pub fn player<S: Into<String>>(message: S) -> Self {
        Self::Player {
            message: message.into(),
        }
    }

    /// Create a new invalid command error
    pub fn invalid_command<S: Into<String>>(reason: S) -> Self {
        Self::InvalidCommand {
            reason: reason.into(),
        }
    }

    /// Create a new frame sync error
    pub fn frame_sync<S: Into<String>>(message: S) -> Self {
        Self::FrameSync {
            message: message.into(),
        }
    }

    /// Create a new packet error
    pub fn packet<S: Into<String>>(reason: S) -> Self {
        Self::InvalidPacket {
            reason: reason.into(),
        }
    }

    /// Create a new serialization error
    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::Serialization(bincode::Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )))
    }

    /// Create a new deserialization error
    pub fn deserialization<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: format!("Deserialization error: {}", message.into()),
        }
    }

    /// Create a new compression error
    pub fn compression<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: format!("Compression error: {}", message.into()),
        }
    }

    /// Create a new decompression error
    pub fn decompression<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: format!("Decompression error: {}", message.into()),
        }
    }

    /// Create an unsupported operation error
    pub fn unsupported<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: format!("Unsupported operation: {}", message.into()),
        }
    }

    /// Create a NAT traversal error
    pub fn nat<S: Into<String>>(message: S) -> Self {
        Self::Nat {
            message: message.into(),
        }
    }

    /// Create a new file transfer error
    pub fn file_transfer<S: Into<String>>(message: S) -> Self {
        Self::FileTransfer {
            message: message.into(),
        }
    }

    /// Create a new security error
    pub fn security<S: Into<String>>(message: S) -> Self {
        Self::Security {
            message: message.into(),
        }
    }

    /// Create a new anti-cheat violation
    pub fn anti_cheat<S: Into<String>>(violation: S) -> Self {
        Self::AntiCheat {
            violation: violation.into(),
        }
    }

    /// Create a new transport error
    pub fn transport<S: Into<String>>(message: S) -> Self {
        Self::Transport {
            message: message.into(),
        }
    }

    /// Create a new matchmaking error
    pub fn matchmaking<S: Into<String>>(message: S) -> Self {
        Self::Matchmaking {
            message: message.into(),
        }
    }

    /// Create a new configuration error (corrected)
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a new resource exhausted error
    pub fn resource_exhausted<S: Into<String>>(resource: S) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
        }
    }

    /// Create a new rate limited error
    pub fn rate_limited<S: Into<String>>(limit: S) -> Self {
        Self::RateLimited {
            limit: limit.into(),
        }
    }

    /// Create a new invalid packet error
    pub fn invalid_packet<S: Into<String>>(reason: S) -> Self {
        Self::InvalidPacket {
            reason: reason.into(),
        }
    }

    /// Create a new protocol mismatch error
    pub fn protocol_mismatch(expected: u32, actual: u32) -> Self {
        Self::ProtocolMismatch { expected, actual }
    }

    /// Create a generic network error
    pub fn generic<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }

    /// Create an invalid state error
    pub fn invalid_state<S: Into<String>>(message: S) -> Self {
        Self::Generic {
            message: format!("Invalid state: {}", message.into()),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Connection issues might be temporary
            Self::Connection { .. } | Self::Timeout(_) | Self::Io(_) => true,

            // Rate limiting is usually temporary
            Self::RateLimited { .. } => true,

            // Resource exhaustion might be temporary
            Self::ResourceExhausted { .. } => true,

            // Protocol errors are usually not recoverable
            Self::ProtocolMismatch { .. } | Self::InvalidPacket { .. } => false,

            // Security violations are never recoverable
            Self::Security { .. } | Self::AntiCheat { .. } => false,

            // Command and frame errors usually indicate bugs
            Self::InvalidCommand { .. } | Self::FrameSync { .. } => false,

            // Other errors depend on context
            _ => false,
        }
    }

    /// Check if this error should cause immediate disconnection
    pub fn should_disconnect(&self) -> bool {
        match self {
            // Security violations always disconnect
            Self::Security { .. } | Self::AntiCheat { .. } => true,

            // Protocol mismatches disconnect
            Self::ProtocolMismatch { .. } => true,

            // Frame sync issues might require disconnect
            Self::FrameSync { .. } => true,

            // Other errors don't necessarily require disconnect
            _ => false,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Security { .. } | Self::AntiCheat { .. } => ErrorSeverity::Critical,
            Self::ProtocolMismatch { .. } | Self::FrameSync { .. } => ErrorSeverity::High,
            Self::Connection { .. } | Self::Player { .. } => ErrorSeverity::Medium,
            Self::Timeout(_) | Self::RateLimited { .. } => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }
}

/// Error severity levels for logging and handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Low severity - might be expected (timeouts, rate limits)
    Low,
    /// Medium severity - should be logged and handled
    Medium,
    /// High severity - requires immediate attention
    High,
    /// Critical severity - security violations, corruption
    Critical,
}

impl ErrorSeverity {
    /// Convert to tracing log level
    pub fn to_log_level(self) -> tracing::Level {
        match self {
            Self::Low => tracing::Level::DEBUG,
            Self::Medium => tracing::Level::WARN,
            Self::High => tracing::Level::ERROR,
            Self::Critical => tracing::Level::ERROR,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = NetworkError::connection("test connection error");
        assert!(matches!(err, NetworkError::Connection { .. }));
        assert!(err.is_recoverable());
        assert!(!err.should_disconnect());
        assert_eq!(err.severity(), ErrorSeverity::Medium);
    }

    #[test]
    fn test_security_error_properties() {
        let err = NetworkError::security("unauthorized access");
        assert!(!err.is_recoverable());
        assert!(err.should_disconnect());
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_anti_cheat_error() {
        let err = NetworkError::anti_cheat("speed hack detected");
        assert!(!err.is_recoverable());
        assert!(err.should_disconnect());
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_protocol_mismatch_error() {
        let err = NetworkError::protocol_mismatch(1, 2);
        assert!(!err.is_recoverable());
        assert!(err.should_disconnect());
        assert_eq!(err.severity(), ErrorSeverity::High);
    }

    #[test]
    fn test_error_severity_log_levels() {
        assert_eq!(ErrorSeverity::Low.to_log_level(), tracing::Level::DEBUG);
        assert_eq!(ErrorSeverity::Medium.to_log_level(), tracing::Level::WARN);
        assert_eq!(ErrorSeverity::High.to_log_level(), tracing::Level::ERROR);
        assert_eq!(
            ErrorSeverity::Critical.to_log_level(),
            tracing::Level::ERROR
        );
    }
}
