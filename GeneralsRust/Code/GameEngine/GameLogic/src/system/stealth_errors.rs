//! Custom error types for the Stealth & Detection System
//!
//! Provides structured, idiomatic Rust error handling for stealth detection,
//! manager operations, and related functionality. Replaces generic `Result<T, String>`
//! with the more expressive `StealthResult<T>` type alias using `StealthError`.
//!
//! All errors implement the standard `std::error::Error` trait and can be
//! propagated using the `?` operator.

use crate::common::ObjectID;
use std::error::Error;
use std::fmt;
use std::sync::PoisonError;

/// Convenience type alias for fallible stealth system APIs
pub type StealthResult<T> = Result<T, StealthError>;

/// Custom error enumeration for stealth and detection operations
///
/// Provides specific error variants for different failure modes, allowing
/// callers to handle specific error cases with appropriate context.
#[derive(Debug, Clone)]
pub enum StealthError {
    /// Object is not registered in the stealth management system
    ObjectNotRegistered {
        /// The object ID that could not be found
        object_id: ObjectID,
    },

    /// Invalid player ID provided
    InvalidPlayerId {
        /// The invalid player ID supplied
        player_id: usize,
        /// The maximum valid player ID
        max: usize,
    },

    /// Invalid or malformed condition specified
    InvalidCondition {
        /// Description of the invalid condition
        condition: String,
    },

    /// Stealth condition prevents the requested operation
    StealthConditionNotMet {
        /// The object unable to maintain stealth
        object_id: ObjectID,
        /// Reason why the condition was not met
        reason: String,
    },

    /// Detection operation failed to complete
    DetectionFailed {
        /// The detector object
        detector_id: ObjectID,
        /// Reason for detection failure
        reason: String,
    },

    /// Requested upgrade was not found
    UpgradeNotFound {
        /// Name of the upgrade being requested
        upgrade_name: String,
    },

    /// Black market facility required but not available
    BlackMarketRequired {
        /// Name of the upgrade requiring black market
        upgrade_name: String,
    },

    /// Disguise transition currently in progress
    DisguiseTransitionInProgress {
        /// The object currently transitioning
        object_id: ObjectID,
    },

    /// Event queue is full and cannot accept new events
    EventQueueFull,

    /// Mutex/lock was poisoned due to a panic in another thread
    LockPoisoned {
        /// The name of the module where the lock was poisoned
        module: String,
    },

    /// Invalid configuration data encountered
    InvalidConfiguration {
        /// The field name that has invalid configuration
        field: String,
        /// Description of what makes the configuration invalid
        reason: String,
    },

    /// Generic operation failure with context
    OperationFailed(String),
}

impl fmt::Display for StealthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StealthError::ObjectNotRegistered { object_id } => {
                write!(f, "object not registered: ID {}", object_id)
            }
            StealthError::InvalidPlayerId { player_id, max } => {
                write!(f, "invalid player ID: {} (max: {})", player_id, max)
            }
            StealthError::InvalidCondition { condition } => {
                write!(f, "invalid stealth condition: {}", condition)
            }
            StealthError::StealthConditionNotMet { object_id, reason } => {
                write!(
                    f,
                    "stealth condition not met for object {}: {}",
                    object_id, reason
                )
            }
            StealthError::DetectionFailed {
                detector_id,
                reason,
            } => {
                write!(
                    f,
                    "detection failed for detector {}: {}",
                    detector_id, reason
                )
            }
            StealthError::UpgradeNotFound { upgrade_name } => {
                write!(f, "upgrade not found: {}", upgrade_name)
            }
            StealthError::BlackMarketRequired { upgrade_name } => {
                write!(f, "black market required for upgrade: {}", upgrade_name)
            }
            StealthError::DisguiseTransitionInProgress { object_id } => {
                write!(
                    f,
                    "disguise transition in progress for object {}",
                    object_id
                )
            }
            StealthError::EventQueueFull => {
                write!(f, "event queue is full")
            }
            StealthError::LockPoisoned { module } => {
                write!(f, "lock poisoned in module: {}", module)
            }
            StealthError::InvalidConfiguration { field, reason } => {
                write!(f, "invalid configuration for field '{}': {}", field, reason)
            }
            StealthError::OperationFailed(msg) => {
                write!(f, "operation failed: {}", msg)
            }
        }
    }
}

impl Error for StealthError {}

impl From<String> for StealthError {
    fn from(err: String) -> Self {
        StealthError::OperationFailed(err)
    }
}

impl PartialEq for StealthError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                StealthError::ObjectNotRegistered { object_id: a },
                StealthError::ObjectNotRegistered { object_id: b },
            ) => a == b,
            (
                StealthError::InvalidPlayerId {
                    player_id: a,
                    max: max_a,
                },
                StealthError::InvalidPlayerId {
                    player_id: b,
                    max: max_b,
                },
            ) => a == b && max_a == max_b,
            (
                StealthError::InvalidCondition { condition: a },
                StealthError::InvalidCondition { condition: b },
            ) => a == b,
            (
                StealthError::StealthConditionNotMet {
                    object_id: a,
                    reason: reason_a,
                },
                StealthError::StealthConditionNotMet {
                    object_id: b,
                    reason: reason_b,
                },
            ) => a == b && reason_a == reason_b,
            (
                StealthError::DetectionFailed {
                    detector_id: a,
                    reason: reason_a,
                },
                StealthError::DetectionFailed {
                    detector_id: b,
                    reason: reason_b,
                },
            ) => a == b && reason_a == reason_b,
            (
                StealthError::UpgradeNotFound { upgrade_name: a },
                StealthError::UpgradeNotFound { upgrade_name: b },
            ) => a == b,
            (
                StealthError::BlackMarketRequired { upgrade_name: a },
                StealthError::BlackMarketRequired { upgrade_name: b },
            ) => a == b,
            (
                StealthError::DisguiseTransitionInProgress { object_id: a },
                StealthError::DisguiseTransitionInProgress { object_id: b },
            ) => a == b,
            (StealthError::EventQueueFull, StealthError::EventQueueFull) => true,
            (
                StealthError::LockPoisoned { module: a },
                StealthError::LockPoisoned { module: b },
            ) => a == b,
            (
                StealthError::InvalidConfiguration {
                    field: a,
                    reason: reason_a,
                },
                StealthError::InvalidConfiguration {
                    field: b,
                    reason: reason_b,
                },
            ) => a == b && reason_a == reason_b,
            (StealthError::OperationFailed(a), StealthError::OperationFailed(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for StealthError {}

// Error conversion implementations
impl<T> From<PoisonError<T>> for StealthError
where
    T: fmt::Debug,
{
    fn from(err: PoisonError<T>) -> Self {
        StealthError::LockPoisoned {
            module: format!("stealth_system: {:?}", err),
        }
    }
}

// Helper functions for common error patterns
impl StealthError {
    /// Creates an `ObjectNotRegistered` error for the given object ID
    pub fn object_not_registered(object_id: ObjectID) -> Self {
        StealthError::ObjectNotRegistered { object_id }
    }

    /// Creates an `InvalidPlayerId` error with max player count
    pub fn invalid_player_id(player_id: usize, max: usize) -> Self {
        StealthError::InvalidPlayerId { player_id, max }
    }

    /// Creates an `InvalidCondition` error
    pub fn invalid_condition(condition: impl Into<String>) -> Self {
        StealthError::InvalidCondition {
            condition: condition.into(),
        }
    }

    /// Creates a `StealthConditionNotMet` error
    pub fn stealth_condition_not_met(object_id: ObjectID, reason: impl Into<String>) -> Self {
        StealthError::StealthConditionNotMet {
            object_id,
            reason: reason.into(),
        }
    }

    /// Creates a `DetectionFailed` error
    pub fn detection_failed(detector_id: ObjectID, reason: impl Into<String>) -> Self {
        StealthError::DetectionFailed {
            detector_id,
            reason: reason.into(),
        }
    }

    /// Creates an `UpgradeNotFound` error
    pub fn upgrade_not_found(upgrade_name: impl Into<String>) -> Self {
        StealthError::UpgradeNotFound {
            upgrade_name: upgrade_name.into(),
        }
    }

    /// Creates a `BlackMarketRequired` error
    pub fn black_market_required(upgrade_name: impl Into<String>) -> Self {
        StealthError::BlackMarketRequired {
            upgrade_name: upgrade_name.into(),
        }
    }

    /// Creates a `DisguiseTransitionInProgress` error
    pub fn disguise_transition_in_progress(object_id: ObjectID) -> Self {
        StealthError::DisguiseTransitionInProgress { object_id }
    }

    /// Creates an `InvalidConfiguration` error
    pub fn invalid_configuration(field: impl Into<String>, reason: impl Into<String>) -> Self {
        StealthError::InvalidConfiguration {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Creates an `OperationFailed` error
    pub fn operation_failed(msg: impl Into<String>) -> Self {
        StealthError::OperationFailed(msg.into())
    }

    /// Adds context to an error, returning a new error with additional information
    pub fn with_context(self, context: impl Into<String>) -> Self {
        let context = context.into();
        match self {
            StealthError::StealthConditionNotMet { object_id, reason } => {
                StealthError::StealthConditionNotMet {
                    object_id,
                    reason: format!("{}: {}", context, reason),
                }
            }
            StealthError::DetectionFailed {
                detector_id,
                reason,
            } => StealthError::DetectionFailed {
                detector_id,
                reason: format!("{}: {}", context, reason),
            },
            StealthError::OperationFailed(msg) => {
                StealthError::OperationFailed(format!("{}: {}", context, msg))
            }
            other => other,
        }
    }
}

/// Extension trait for converting Result<T, String> to Result<T, StealthError>
pub trait StringErrorExt<T> {
    /// Converts a String error to a StealthError using provided converter function
    fn map_string_error<F>(self, f: F) -> StealthResult<T>
    where
        F: FnOnce(String) -> StealthError;
}

impl<T> StringErrorExt<T> for Result<T, String> {
    fn map_string_error<F>(self, f: F) -> StealthResult<T>
    where
        F: FnOnce(String) -> StealthError,
    {
        self.map_err(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_not_registered_error() {
        let error = StealthError::object_not_registered(42);
        assert_eq!(error, StealthError::ObjectNotRegistered { object_id: 42 });
        assert_eq!(error.to_string(), "object not registered: ID 42");
    }

    #[test]
    fn test_invalid_player_id_error() {
        let error = StealthError::invalid_player_id(10, 8);
        assert_eq!(
            error,
            StealthError::InvalidPlayerId {
                player_id: 10,
                max: 8
            }
        );
        assert_eq!(error.to_string(), "invalid player ID: 10 (max: 8)");
    }

    #[test]
    fn test_invalid_condition_error() {
        let error = StealthError::invalid_condition("moving while stealthed");
        assert_eq!(
            error,
            StealthError::InvalidCondition {
                condition: "moving while stealthed".to_string()
            }
        );
        assert!(error.to_string().contains("invalid stealth condition"));
    }

    #[test]
    fn test_stealth_condition_not_met_error() {
        let error = StealthError::stealth_condition_not_met(99, "unit is attacking");
        assert_eq!(
            error,
            StealthError::StealthConditionNotMet {
                object_id: 99,
                reason: "unit is attacking".to_string()
            }
        );
        assert!(error.to_string().contains("object 99"));
        assert!(error.to_string().contains("unit is attacking"));
    }

    #[test]
    fn test_detection_failed_error() {
        let error = StealthError::detection_failed(50, "insufficient detection strength");
        assert_eq!(
            error,
            StealthError::DetectionFailed {
                detector_id: 50,
                reason: "insufficient detection strength".to_string()
            }
        );
        assert!(error.to_string().contains("detector 50"));
    }

    #[test]
    fn test_upgrade_not_found_error() {
        let error = StealthError::upgrade_not_found("Advanced Cloak");
        assert_eq!(
            error,
            StealthError::UpgradeNotFound {
                upgrade_name: "Advanced Cloak".to_string()
            }
        );
        assert!(error.to_string().contains("Advanced Cloak"));
    }

    #[test]
    fn test_black_market_required_error() {
        let error = StealthError::black_market_required("Nuclear Armor");
        assert_eq!(
            error,
            StealthError::BlackMarketRequired {
                upgrade_name: "Nuclear Armor".to_string()
            }
        );
        assert!(error.to_string().contains("black market"));
        assert!(error.to_string().contains("Nuclear Armor"));
    }

    #[test]
    fn test_disguise_transition_in_progress_error() {
        let error = StealthError::disguise_transition_in_progress(75);
        assert_eq!(
            error,
            StealthError::DisguiseTransitionInProgress { object_id: 75 }
        );
        assert!(error.to_string().contains("object 75"));
        assert!(error.to_string().contains("disguise"));
    }

    #[test]
    fn test_event_queue_full_error() {
        let error = StealthError::EventQueueFull;
        assert_eq!(error, StealthError::EventQueueFull);
        assert_eq!(error.to_string(), "event queue is full");
    }

    #[test]
    fn test_lock_poisoned_error() {
        let error = StealthError::LockPoisoned {
            module: "stealth_manager".to_string(),
        };
        assert!(error.to_string().contains("lock poisoned"));
        assert!(error.to_string().contains("stealth_manager"));
    }

    #[test]
    fn test_invalid_configuration_error() {
        let error = StealthError::invalid_configuration("max_objects", "must be positive");
        assert_eq!(
            error,
            StealthError::InvalidConfiguration {
                field: "max_objects".to_string(),
                reason: "must be positive".to_string()
            }
        );
        assert!(error.to_string().contains("max_objects"));
        assert!(error.to_string().contains("must be positive"));
    }

    #[test]
    fn test_error_with_context() {
        let base_error = StealthError::detection_failed(50, "low strength");
        let with_context = base_error.with_context("in visibility check");
        match with_context {
            StealthError::DetectionFailed { reason, .. } => {
                assert!(reason.contains("in visibility check"));
                assert!(reason.contains("low strength"));
            }
            _ => panic!("Expected DetectionFailed error"),
        }
    }

    #[test]
    fn test_error_implements_std_error() {
        let error: Box<dyn std::error::Error> = Box::new(StealthError::EventQueueFull);
        assert_eq!(error.to_string(), "event queue is full");
    }

    #[test]
    fn test_operation_failed_error() {
        let error = StealthError::operation_failed("unknown stealth state");
        match error {
            StealthError::OperationFailed(msg) => {
                assert_eq!(msg, "unknown stealth state");
            }
            _ => panic!("Expected OperationFailed"),
        }
    }

    #[test]
    fn test_display_formatting_all_variants() {
        // Verify all variants format without panicking
        let _ = format!("{}", StealthError::object_not_registered(1));
        let _ = format!("{}", StealthError::invalid_player_id(5, 8));
        let _ = format!("{}", StealthError::invalid_condition("test"));
        let _ = format!("{}", StealthError::stealth_condition_not_met(2, "reason"));
        let _ = format!("{}", StealthError::detection_failed(3, "reason"));
        let _ = format!("{}", StealthError::upgrade_not_found("test"));
        let _ = format!("{}", StealthError::black_market_required("test"));
        let _ = format!("{}", StealthError::disguise_transition_in_progress(4));
        let _ = format!("{}", StealthError::EventQueueFull);
        let _ = format!(
            "{}",
            StealthError::LockPoisoned {
                module: "test".to_string()
            }
        );
        let _ = format!("{}", StealthError::invalid_configuration("field", "reason"));
        let _ = format!("{}", StealthError::operation_failed("msg"));
    }
}
