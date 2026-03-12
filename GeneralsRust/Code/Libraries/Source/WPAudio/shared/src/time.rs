//! Cross-platform timing utilities

pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Convert milliseconds to Duration
pub fn millis_to_duration(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// Convert seconds to Duration  
pub fn seconds_to_duration(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}
