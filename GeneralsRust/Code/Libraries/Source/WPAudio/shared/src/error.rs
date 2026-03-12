//! Common error types for WestWood Studios library conversions

use thiserror::Error;

/// Shared error type for all WestWood Studios library conversions
#[derive(Debug, Error)]
pub enum SharedError {
    /// I/O related errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Platform-specific errors
    #[error("Platform error: {message}")]
    Platform {
        /// Error message
        message: String,
    },

    /// Memory allocation errors
    #[error("Memory error: {message}")]
    Memory {
        /// Error message
        message: String,
    },

    /// Threading/synchronization errors  
    #[error("Threading error: {message}")]
    Threading {
        /// Error message
        message: String,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        /// Error message
        message: String,
    },

    /// Generic errors with context
    #[error("Error: {message}")]
    Generic {
        /// Error message
        message: String,
    },
}

impl SharedError {
    /// Create a platform-specific error
    pub fn platform(message: impl Into<String>) -> Self {
        Self::Platform {
            message: message.into(),
        }
    }

    /// Create a memory-related error
    pub fn memory(message: impl Into<String>) -> Self {
        Self::Memory {
            message: message.into(),
        }
    }

    /// Create a threading-related error
    pub fn threading(message: impl Into<String>) -> Self {
        Self::Threading {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Self::Generic {
            message: message.into(),
        }
    }
}
