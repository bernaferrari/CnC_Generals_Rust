//! Logic-frame lifetime scheduling and expiration state.

use crate::time::LOGIC_FRAMES_PER_SECOND;
use serde::{Deserialize, Serialize};

/// Validated authored LifetimeUpdate or DeletionUpdate bounds in logic frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LifetimeFrameRange {
    pub minimum: u32,
    pub maximum: u32,
}

impl LifetimeFrameRange {
    /// LifetimeUpdate.cpp:59–64 samples an inclusive range, clamps delay to
    /// at least one frame, and adds it to the unsigned logic frame.
    pub fn deadline(self, frame: u32, random_word: u32) -> u32 {
        let width = u64::from(self.maximum) - u64::from(self.minimum) + 1;
        let delay = u64::from(self.minimum) + u64::from(random_word) % width;
        frame.wrapping_add((delay as u32).max(1))
    }
}

/// Lifetime for content without an authored update module. The durable fields
/// match the previous snapshot representation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LifetimeTimer {
    pub expire_at_frame: u32,
    pub active: bool,
}

impl LifetimeTimer {
    pub fn from_delay_frames(frame: u32, delay: u32) -> Self {
        Self {
            expire_at_frame: frame.saturating_add(delay.max(1)),
            active: true,
        }
    }

    /// The unbound template fallback rounds millisecond ranges and receives
    /// its sampler explicitly. An owning hulk override replaces both bounds.
    pub fn from_msec_range(
        frame: u32,
        (min_msec, max_msec): (u32, u32),
        hulk_override_frames: Option<u32>,
        sample: impl FnOnce(i32, i32) -> i32,
    ) -> Self {
        let (min_msec, max_msec) = match hulk_override_frames {
            Some(frames) => {
                let msec = (frames as f32 * 1000.0 / LOGIC_FRAMES_PER_SECOND).round() as u32;
                (msec, msec)
            }
            None => (min_msec.min(max_msec), min_msec.max(max_msec)),
        };
        let frames = |msec| (msec as f32 * LOGIC_FRAMES_PER_SECOND / 1000.0).round() as u32;
        let min = frames(min_msec).max(1);
        let max = frames(max_msec).max(min);
        let delay = sample(min as i32, max as i32).max(1) as u32;
        Self::from_delay_frames(frame, delay)
    }

    pub fn take_expiration(&mut self, frame: u32) -> bool {
        if !self.active || self.expire_at_frame == 0 || frame < self.expire_at_frame {
            return false;
        }
        self.active = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_range_is_inclusive_and_deadline_wraps() {
        let range = LifetimeFrameRange {
            minimum: 3,
            maximum: 7,
        };
        assert_eq!(range.deadline(100, 0), 103);
        assert_eq!(range.deadline(100, 4), 107);
        assert_eq!(range.deadline(u32::MAX - 1, 4), 5);
        assert_eq!(LifetimeFrameRange::default().deadline(100, u32::MAX), 101);
    }

    #[test]
    fn fallback_hulk_override_sets_both_bounds_and_expires_once() {
        let range = (10_000, 20_000);
        let mut timer = LifetimeTimer::from_msec_range(7, range, Some(30), |min, max| {
            assert_eq!((min, max), (30, 30));
            min
        });
        assert!(!timer.take_expiration(36));
        assert!(timer.take_expiration(37));
        assert!(!timer.take_expiration(37));
        let ordinary = LifetimeTimer::from_msec_range(7, (range.1, range.0), None, |min, max| {
            assert_eq!((min, max), (300, 600));
            max
        });
        assert_eq!(ordinary.expire_at_frame, 607);
    }
}
