//! Monotonic networking clock matching the C++ `timeGetTime()` behavior.
//!
//! The original GameNetwork stack relies on `timeGetTime()` (milliseconds since system
//! start, wrapping at 2^32) for keep-alives, disconnect timers, and diagnostics.  This
//! module provides an equivalent abstraction so the Rust port no longer reaches directly
//! for `Instant::now()`, guaranteeing deterministic timing across platforms.

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Monotonic timestamp used throughout the GameNetwork crate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkInstant {
    since_start: Duration,
}

impl NetworkInstant {
    /// Returns the current timestamp.
    #[inline]
    pub fn now() -> Self {
        if EXTERNAL_TIME_ACTIVE.load(Ordering::Relaxed) {
            let nanos = EXTERNAL_TIME_NANOS.load(Ordering::Relaxed);
            return Self::from_duration(Duration::from_nanos(nanos));
        }
        Self {
            since_start: clock_start().elapsed(),
        }
    }

    /// Builds a timestamp from a raw duration (primarily for tests).
    #[inline]
    pub fn from_duration(duration: Duration) -> Self {
        Self {
            since_start: duration,
        }
    }

    /// Returns the elapsed time since the networking clock started.
    #[inline]
    pub fn as_duration(&self) -> Duration {
        self.since_start
    }

    /// Returns milliseconds since start, wrapping at the 32-bit boundary just like `timeGetTime()`.
    #[inline]
    pub fn as_timegettime_ticks(&self) -> u32 {
        let millis = self.since_start.as_millis() as u64;
        (millis % (u32::MAX as u64 + 1)) as u32
    }

    /// Duration from `earlier` to `self`, saturating at zero.
    #[inline]
    pub fn duration_since(&self, earlier: NetworkInstant) -> Duration {
        self.since_start.saturating_sub(earlier.since_start)
    }

    /// Returns the time elapsed since this timestamp.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        NetworkInstant::now()
            .since_start
            .saturating_sub(self.since_start)
    }

    /// Returns `Some(t)` if adding `duration` does not overflow.
    #[inline]
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.since_start
            .checked_add(duration)
            .map(|since_start| Self { since_start })
    }

    /// Returns `Some(t)` if subtracting `duration` does not go negative.
    #[inline]
    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        self.since_start
            .checked_sub(duration)
            .map(|since_start| Self { since_start })
    }
}

impl Add<Duration> for NetworkInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        NetworkInstant {
            since_start: self.since_start + rhs,
        }
    }
}

impl AddAssign<Duration> for NetworkInstant {
    fn add_assign(&mut self, rhs: Duration) {
        self.since_start += rhs;
    }
}

impl Sub<Duration> for NetworkInstant {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        NetworkInstant {
            since_start: self.since_start.saturating_sub(rhs),
        }
    }
}

impl SubAssign<Duration> for NetworkInstant {
    fn sub_assign(&mut self, rhs: Duration) {
        self.since_start = self.since_start.saturating_sub(rhs);
    }
}

/// Global networking clock helper.
pub struct NetworkClock;

impl NetworkClock {
    /// Returns the current timestamp.
    #[inline]
    pub fn now() -> NetworkInstant {
        NetworkInstant::now()
    }

    /// Returns milliseconds since start, matching `timeGetTime()`.
    #[inline]
    pub fn now_timegettime() -> u32 {
        NetworkInstant::now().as_timegettime_ticks()
    }

    /// Overrides the networking clock with an externally supplied absolute time.
    ///
    /// This is intended for synchronizing with the WW3D `FrameTiming` so that networking
    /// components use the same notion of "now" as the renderer/game loop.
    pub fn override_with_duration(duration: Duration) {
        let nanos = duration.as_nanos().min(u64::MAX as u128) as u64;
        EXTERNAL_TIME_NANOS.store(nanos, Ordering::Relaxed);
        EXTERNAL_TIME_ACTIVE.store(true, Ordering::Relaxed);
    }

    /// Clears any external override so the clock reverts to host monotonic time.
    pub fn clear_override() {
        EXTERNAL_TIME_ACTIVE.store(false, Ordering::Relaxed);
    }
}

fn clock_start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

static EXTERNAL_TIME_ACTIVE: AtomicBool = AtomicBool::new(false);
static EXTERNAL_TIME_NANOS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timegettime_wraps_u32_range() {
        let wrap_base = Duration::from_millis(u32::MAX as u64 + 123);
        let instant = NetworkInstant::from_duration(wrap_base);
        assert_eq!(instant.as_timegettime_ticks(), 122);
    }

    #[test]
    fn duration_math_matches_added_offsets() {
        let base = NetworkInstant::from_duration(Duration::from_millis(10_000));
        let later = base + Duration::from_millis(250);
        assert_eq!(later.duration_since(base), Duration::from_millis(250));

        let earlier = base - Duration::from_millis(5_000);
        assert_eq!(base.duration_since(earlier), Duration::from_millis(5_000));
    }

    #[test]
    fn override_clock_uses_external_time() {
        NetworkClock::override_with_duration(Duration::from_secs(5));
        let now = NetworkInstant::now();
        assert_eq!(now.as_duration(), Duration::from_secs(5));
        NetworkClock::clear_override();
    }
}
