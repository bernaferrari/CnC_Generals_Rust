//! Audio timing module - Rust conversion of AUD_Time.cpp
//!
//! This module provides high-resolution timing functionality for audio processing,
//! originally implemented in C++ using Windows-specific APIs. This Rust version
//! provides cross-platform timing while maintaining the same interface and behavior.
//!
//! ## Original C++ Description
//!
//! **Project:** Dune Emperor  
//! **Module:** Audio Timer (AUD_)  
//! **File:** audtimer.cpp  
//! **Created by:** 04/??/99 TR  
//!
//! The original implementation used Windows QueryPerformanceCounter for high-resolution
//! timing with a fallback to timeGetTime() for systems without high-resolution counters.
//! This Rust version uses std::time::Instant for cross-platform high-resolution timing.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};

/// TimeStamp represents a point in time, compatible with the original C++ implementation.
/// In the original C++, this was a 64-bit integer representing milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeStamp {
    /// Milliseconds since initialization
    millis: u64,
}

impl TimeStamp {
    /// Zero timestamp (equivalent to C++ static initialization)
    pub const ZERO: TimeStamp = TimeStamp { millis: 0 };

    /// Create a new timestamp from milliseconds
    pub const fn from_millis(millis: u64) -> Self {
        Self { millis }
    }

    /// Get the timestamp value in milliseconds
    pub const fn as_millis(&self) -> u64 {
        self.millis
    }

    /// Convert to Duration
    pub const fn as_duration(&self) -> Duration {
        Duration::from_millis(self.millis)
    }

    /// Create from Duration
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            millis: duration.as_millis() as u64,
        }
    }
}

impl std::ops::Add<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: TimeStamp) -> TimeStamp {
        TimeStamp {
            millis: self.millis.saturating_add(other.millis),
        }
    }
}

impl std::ops::Sub<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn sub(self, other: TimeStamp) -> TimeStamp {
        TimeStamp {
            millis: self.millis.saturating_sub(other.millis),
        }
    }
}

/// Timer function type - equivalent to the C++ function pointer
type TimerFunc = fn() -> TimeStamp;

/// Static timer state, equivalent to the original C++ static variables
struct TimerState {
    _last_time: TimeStamp,
    _interval: TimeStamp,
    _timeout: TimeStamp,
    timer_func: Option<TimerFunc>,
    #[cfg(windows)]
    timer_millis_scale: u64,
}

static TIMER_STATE: OnceLock<Mutex<TimerState>> = OnceLock::new();

/// Initialize the timer state
fn get_timer_state() -> &'static Mutex<TimerState> {
    TIMER_STATE.get_or_init(|| {
        Mutex::new(TimerState {
            _last_time: TimeStamp::ZERO,
            _interval: TimeStamp::ZERO,
            _timeout: TimeStamp::ZERO,
            timer_func: None,
            #[cfg(windows)]
            timer_millis_scale: 0,
        })
    })
}

/// High-resolution timer implementation (equivalent to highResGetTime)
///
/// On modern systems, std::time::Instant provides high-resolution timing
/// across all platforms, so we use it directly.
fn high_res_get_time() -> TimeStamp {
    static START_TIME: OnceLock<Instant> = OnceLock::new();

    let start = START_TIME.get_or_init(Instant::now);
    let elapsed = start.elapsed();

    TimeStamp::from_duration(elapsed)
}

/// Failsafe timer implementation (equivalent to failsafeGetTime)
///
/// This provides a thread-safe fallback timer that handles potential
/// wraparound issues, similar to the original C++ implementation.
#[cfg(windows)]
#[allow(dead_code)] // Conditional compilation: only compiled on Windows
fn failsafe_get_time() -> TimeStamp {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    // Static variables equivalent to the C++ implementation
    static TIME: AtomicU64 = AtomicU64::new(0x100000000); // Initial high value
    static CALLS: AtomicU32 = AtomicU32::new(0);

    let mut calls = CALLS.fetch_add(1, Ordering::SeqCst);

    loop {
        let called = calls;
        let current_time = TIME.load(Ordering::Acquire);

        // Extract high and low words (equivalent to C++ pointer manipulation)
        let hw = (current_time >> 32) as u32;
        let lw = (current_time & 0xFFFFFFFF) as u32;

        // Check for re-entry
        if called != CALLS.load(Ordering::Acquire) {
            calls = CALLS.load(Ordering::Acquire);
            continue;
        }

        // Get current system time (equivalent to timeGetTime())
        // On Windows, we can use GetTickCount64 for better precision
        let now = unsafe { winapi::um::sysinfoapi::GetTickCount64() as u32 };

        let mut new_hw = hw;
        if now < lw {
            // Handle wraparound
            new_hw = new_hw.saturating_add(1);
        }

        let new_time = ((new_hw as u64) << 32) | (now as u64);

        // Try to update atomically
        if TIME
            .compare_exchange(current_time, new_time, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            // Check again for re-entry after update
            if called == CALLS.load(Ordering::Acquire) {
                return TimeStamp::from_millis(new_time);
            }
        }

        // Retry if we were re-entered
        calls = CALLS.load(Ordering::Acquire);
    }
}

/// Cross-platform failsafe timer for non-Windows systems
#[cfg(not(windows))]
#[allow(dead_code)] // Conditional compilation: only compiled on non-Windows
fn failsafe_get_time() -> TimeStamp {
    // On non-Windows systems, std::time::Instant is already high-resolution
    // and handles all edge cases, so we can just use it directly
    high_res_get_time()
}

/// Initialize the audio timer (equivalent to InitAudioTimer)
///
/// This function initializes the high-resolution timer by determining the best
/// available timing mechanism. On modern systems, std::time::Instant provides
/// high-resolution timing across all platforms.
///
/// # Original C++ Description
///
/// Initialize the high resolution timer by querying the system for its
/// availability. If one does exist then we set the game timer function
/// to 'highResGetTime' otherwise we use the original code at 'failsafeGetTime'.
/// For the hi res counter we precalculate the millisecond scaling factor to
/// convert hi res ticks to millisecond usage.
pub fn init_audio_timer() {
    let state = get_timer_state();
    let mut guard = state.lock().unwrap();

    // In Rust, std::time::Instant is always high-resolution when available
    // We can check the resolution to decide which timer to use
    #[cfg(windows)]
    {
        // On Windows, we can use QueryPerformanceFrequency equivalent
        use std::time::Instant;

        let start = Instant::now();
        std::thread::sleep(Duration::from_nanos(1));
        let elapsed = start.elapsed();

        if elapsed.as_nanos() > 0 && elapsed.as_nanos() < 1_000_000 {
            // High resolution timer available (sub-millisecond precision)
            guard.timer_func = Some(high_res_get_time);
        } else {
            // Fall back to failsafe timer
            guard.timer_func = Some(failsafe_get_time);
        }
    }

    #[cfg(not(windows))]
    {
        // On non-Windows systems, Instant is always high-resolution when available
        guard.timer_func = Some(high_res_get_time);
    }
}

/// Get the current audio time (equivalent to AudioGetTime)
///
/// Returns the current timestamp using the initialized timer function.
/// If the timer hasn't been initialized, returns zero.
///
/// # Returns
///
/// Current timestamp in milliseconds since timer initialization
pub fn audio_get_time() -> TimeStamp {
    let state = get_timer_state();
    let guard = state.lock().unwrap();

    match guard.timer_func {
        Some(func) => func(),
        None => TimeStamp::ZERO,
    }
}

/// Utility macro to convert seconds to milliseconds (equivalent to SECONDS macro)
///
/// # Example
/// ```rust
/// use wp_audio::{seconds, TimeStamp};
/// let one_second: TimeStamp = seconds!(1);
/// let half_second: TimeStamp = seconds!(0.5);
/// ```
#[macro_export]
macro_rules! seconds {
    ($secs:expr) => {
        TimeStamp::from_millis((($secs as f64) * 1000.0) as u64)
    };
}

/// Utility macro to convert milliseconds to TimeStamp (equivalent to MSECONDS macro)
#[macro_export]
macro_rules! mseconds {
    ($ms:expr) => {
        TimeStamp::from_millis($ms as u64)
    };
}

/// Utility macro to convert TimeStamp to seconds (equivalent to IN_SECONDS macro)
#[macro_export]
macro_rules! in_seconds {
    ($timestamp:expr) => {
        ($timestamp.as_millis() as f64) / 1000.0
    };
}

/// Utility macro to convert TimeStamp to milliseconds (equivalent to IN_MSECONDS macro)
#[macro_export]
macro_rules! in_mseconds {
    ($timestamp:expr) => {
        $timestamp.as_millis()
    };
}

// Re-export the macros for easier use
pub use {in_mseconds, in_seconds, mseconds, seconds};

/// Error types for timer operations
#[derive(Debug, thiserror::Error)]
pub enum TimerError {
    #[error("Timer not initialized - call init_audio_timer() first")]
    NotInitialized,
    #[error("Timer operation failed: {message}")]
    OperationFailed { message: String },
}

/// Result type for timer operations
pub type TimerResult<T> = Result<T, TimerError>;

/// Advanced timer utilities for audio processing
pub mod utils {
    use super::*;

    /// Get timer resolution information
    pub fn get_timer_info() -> TimerInfo {
        let start = std::time::Instant::now();
        std::thread::sleep(Duration::from_nanos(1));
        let min_resolution = start.elapsed();

        TimerInfo {
            has_high_resolution: min_resolution.as_nanos() < 1_000_000, // < 1ms
            resolution_nanos: min_resolution.as_nanos() as u64,
            is_monotonic: true, // std::time::Instant is always monotonic
        }
    }

    /// Timer information structure
    #[derive(Debug, Clone)]
    pub struct TimerInfo {
        pub has_high_resolution: bool,
        pub resolution_nanos: u64,
        pub is_monotonic: bool,
    }

    /// Calculate elapsed time between two timestamps
    pub fn elapsed_time(start: TimeStamp, end: TimeStamp) -> Duration {
        if end >= start {
            Duration::from_millis(end.as_millis() - start.as_millis())
        } else {
            Duration::ZERO
        }
    }

    /// Check if a timeout has elapsed
    pub fn is_timeout_elapsed(start: TimeStamp, timeout: Duration) -> bool {
        let current = super::audio_get_time();
        let elapsed = elapsed_time(start, current);
        elapsed >= timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_timer_initialization() {
        init_audio_timer();
        let time1 = audio_get_time();
        assert!(time1.as_millis() >= 0);
    }

    #[test]
    fn test_timestamp_operations() {
        let ts1 = TimeStamp::from_millis(1000);
        let ts2 = TimeStamp::from_millis(500);

        assert_eq!((ts1 + ts2).as_millis(), 1500);
        assert_eq!((ts1 - ts2).as_millis(), 500);
    }

    #[test]
    fn test_macros() {
        let one_sec = seconds!(1);
        assert_eq!(one_sec.as_millis(), 1000);

        let five_hundred_ms = mseconds!(500);
        assert_eq!(five_hundred_ms.as_millis(), 500);

        assert_eq!(in_seconds!(one_sec), 1.0);
        assert_eq!(in_mseconds!(five_hundred_ms), 500);
    }

    #[test]
    fn test_timer_progression() {
        init_audio_timer();
        let start = audio_get_time();

        thread::sleep(Duration::from_millis(10));

        let end = audio_get_time();
        assert!(end >= start);

        let elapsed = utils::elapsed_time(start, end);
        assert!(elapsed.as_millis() >= 8); // Allow some tolerance for timing
    }

    #[test]
    fn test_timeout_detection() {
        init_audio_timer();
        let start = audio_get_time();
        let short_timeout = Duration::from_millis(1);

        thread::sleep(Duration::from_millis(5));

        assert!(utils::is_timeout_elapsed(start, short_timeout));
    }

    #[test]
    fn test_timer_info() {
        let info = utils::get_timer_info();
        assert!(info.resolution_nanos > 0);
        assert!(info.is_monotonic);
    }
}

// Optional Windows-specific dependencies
#[cfg(windows)]
mod windows_deps {
    // If we need Windows-specific timing, we can add winapi dependency
    // For now, std::time::Instant provides sufficient functionality
}
