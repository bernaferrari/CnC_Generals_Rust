//! Unified error types for the GameLogic crate.
//!
//! The original C++ implementation surfaces many failure codes via asserts and
//! global flags.  The Rust port adopts a structured error model so callers can
//! propagate failures with context.

use crate::common::ObjectID;
use std::fmt;

/// Convenience alias for fallible GameLogic APIs.
pub type GameLogicResult<T> = Result<T, GameLogicError>;

/// Canonical error enumeration for the GameLogic crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameLogicError {
    /// Invalid configuration data (usually INI or script content).
    Configuration(String),
    /// Backward-compatible catch-all error for legacy call sites.
    GenericError(String),
    /// Operation was not valid for the current object/state.
    InvalidOperation,
    /// Requested object/template/player could not be found.
    ObjectNotFound(ObjectID),
    /// Identifier referenced an object that is no longer valid.
    InvalidObject(ObjectID),
    /// Failed to acquire or interact with a lock/mutex.
    LockError,
    /// Call required a world position that was not valid.
    InvalidPosition,
    /// Subsystem has not been initialised prior to use.
    SystemNotInitialized(String),
    /// Underlying runtime or platform failure.
    SystemError(String),
    /// Mutex/lock poisoning or other threading issues.
    Threading(String),
    /// Failure inside a behaviour/update module.
    ModuleError(String),
    /// I/O operation failure.
    IO(String),
}

impl fmt::Display for GameLogicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameLogicError::InvalidOperation => f.write_str("invalid operation"),
            GameLogicError::LockError => f.write_str("lock poisoned"),
            GameLogicError::Configuration(msg) => write!(f, "configuration error: {}", msg),
            GameLogicError::GenericError(msg) => write!(f, "error: {}", msg),
            GameLogicError::ObjectNotFound(id) => write!(f, "object not found: {}", id),
            GameLogicError::InvalidObject(id) => write!(f, "invalid object identifier: {}", id),
            GameLogicError::InvalidPosition => f.write_str("invalid world position"),
            GameLogicError::SystemNotInitialized(msg) => {
                write!(f, "system not initialised: {}", msg)
            }
            GameLogicError::SystemError(msg) => write!(f, "system error: {}", msg),
            GameLogicError::Threading(msg) => write!(f, "threading failure: {}", msg),
            GameLogicError::ModuleError(msg) => write!(f, "module failure: {}", msg),
            GameLogicError::IO(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for GameLogicError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for GameLogicError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        GameLogicError::ModuleError(err.to_string())
    }
}
