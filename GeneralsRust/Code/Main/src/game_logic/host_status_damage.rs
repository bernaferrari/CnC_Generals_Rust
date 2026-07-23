//! Host StatusDamageHelper residual (timed OBJECT_STATUS clear).
//!
//! C++: `StatusDamageHelper::doStatusDamage` sets a status bit and wakes to
//! clear it after duration frames. `update` clears when timer fires.
//!
//! Residual playability slice:
//! - One active timed status residual (last write wins)
//! - Duration from DAMAGE_STATUS amount (msec → frames @ 30 FPS)
//! - Clears status when frame reaches heal frame
//!
//! Fail-closed: not full multi-status stack / all ObjectStatusTypes matrix.

use serde::{Deserialize, Serialize};

pub const STATUS_DAMAGE_LOGIC_FPS: f32 = 30.0;

#[inline]
pub fn status_duration_ms_to_frames(ms: f32) -> u32 {
    ((ms.max(0.0)) * STATUS_DAMAGE_LOGIC_FPS / 1000.0)
        .round()
        .max(1.0) as u32
}

/// Timed status residual kinds that host can set/clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostTimedStatus {
    #[default]
    None,
    /// C++ OBJECT_STATUS_CAN_ATTACK residual peel (rare).
    CanAttackDisabled,
    /// Residual: stealth disabled / detected forced residual.
    ForceDetected,
    /// Residual: weapon-bonus disabled residual.
    WeaponsDisabled,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStatusDamageData {
    pub status: HostTimedStatus,
    pub frame_to_heal: u32,
    pub applications: u32,
}

impl HostStatusDamageData {
    pub fn do_status_damage(&mut self, status: HostTimedStatus, duration_ms: f32, now: u32) {
        if status == HostTimedStatus::None {
            return;
        }
        let frames = status_duration_ms_to_frames(duration_ms);
        self.status = status;
        self.frame_to_heal = now.saturating_add(frames);
        self.applications = self.applications.saturating_add(1);
    }

    /// Returns cleared status if timer elapsed.
    pub fn tick_clear(&mut self, now: u32) -> Option<HostTimedStatus> {
        if self.status == HostTimedStatus::None || self.frame_to_heal == 0 {
            return None;
        }
        if now >= self.frame_to_heal {
            let s = self.status;
            self.status = HostTimedStatus::None;
            self.frame_to_heal = 0;
            return Some(s);
        }
        None
    }

    pub fn is_active(&self) -> bool {
        self.status != HostTimedStatus::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_clears_after_duration() {
        let mut s = HostStatusDamageData::default();
        s.do_status_damage(HostTimedStatus::ForceDetected, 1000.0, 0);
        assert!(s.is_active());
        assert!(s.tick_clear(10).is_none());
        assert_eq!(s.tick_clear(30), Some(HostTimedStatus::ForceDetected));
        assert!(!s.is_active());
    }
}
