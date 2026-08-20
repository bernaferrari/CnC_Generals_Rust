//! Host LifetimeUpdate residual (auto-die after min/max frames).
//!
//! C++: `LifetimeUpdate` sleeps until random frame in [min,max], then kills object
//! (which may trigger CreateObjectDie / FXListDie / etc.).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI32, Ordering};

/// C++ GameLogic::m_scriptHulkMaxLifetimeOverride (default -1 = unused).
static HULK_MAX_LIFETIME_OVERRIDE: AtomicI32 = AtomicI32::new(-1);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostLifetimeUpdateData {
    pub expire_at_frame: u32,
    pub active: bool,
}

impl HostLifetimeUpdateData {
    pub fn from_delay_frames(current_frame: u32, frames: u32) -> Self {
        Self {
            expire_at_frame: current_frame.saturating_add(frames.max(1)),
            active: true,
        }
    }

    pub fn from_msec(current_frame: u32, msec: u32) -> Self {
        let frames = ((msec as f32) * 30.0 / 1000.0).round() as u32;
        Self::from_delay_frames(current_frame, frames.max(1))
    }

    /// C++ LifetimeUpdate.cpp:59-64 GameLogicRandomValue(Min,Max) + HULK override.
    pub fn from_ini_range(
        current_frame: u32,
        min_msec: u32,
        max_msec: u32,
        seed: u32,
        is_hulk: bool,
    ) -> Self {
        let (min_ms, max_ms) = effective_lifetime_msec_range(min_msec, max_msec, is_hulk);
        let min_f = msec_to_frames(min_ms).max(1);
        let max_f = msec_to_frames(max_ms).max(min_f);
        let delay = crate::game_logic::host_rng_residual::pure_logic_random_int(
            seed,
            0,
            min_f as i32,
            max_f as i32,
        )
        .max(1) as u32;
        Self::from_delay_frames(current_frame, delay)
    }

    /// Returns true when lifetime expired this frame.
    pub fn tick(&self, current_frame: u32) -> bool {
        self.active && current_frame >= self.expire_at_frame && self.expire_at_frame > 0
    }
}

fn msec_to_frames(msec: u32) -> u32 {
    ((msec as f32) * 30.0 / 1000.0).round() as u32
}

pub fn set_hulk_max_lifetime_override(frames_or_sentinel: i32) {
    HULK_MAX_LIFETIME_OVERRIDE.store(frames_or_sentinel, Ordering::Relaxed);
}

pub fn get_hulk_max_lifetime_override() -> i32 {
    HULK_MAX_LIFETIME_OVERRIDE.load(Ordering::Relaxed)
}

/// C++ LifetimeUpdate.cpp:31-37 HULK override replaces Min/Max when != -1.
pub fn effective_lifetime_msec_range(min_msec: u32, max_msec: u32, is_hulk: bool) -> (u32, u32) {
    let ov = get_hulk_max_lifetime_override();
    if is_hulk && ov >= 0 {
        let ms = ((ov as f32) * 1000.0 / 30.0).round() as u32;
        return (ms, ms);
    }
    if min_msec <= max_msec {
        (min_msec, max_msec)
    } else {
        (max_msec, min_msec)
    }
}

/// Retail INI MinLifetime / MaxLifetime (msec) for common LifetimeUpdate users.
pub fn lifetime_msec_range_for_template(name: &str) -> Option<(u32, u32)> {
    let n = name.to_ascii_lowercase();
    if n.contains("sneakattack") && n.contains("start") {
        return Some((5_000, 5_000));
    }
    if n.contains("poisonfieldmedium") {
        return Some((30_000, 30_000));
    }
    if n.contains("poisonfieldsmall") {
        return Some((20_000, 20_000));
    }
    if n.contains("poisonfieldlarge") {
        return Some((40_000, 40_000));
    }
    if n.contains("firestorm") {
        return Some((6_000, 6_000));
    }
    if n.contains("tntsticky") || (n.contains("sticky") && n.contains("bomb")) {
        return Some((10_000, 10_000));
    }
    if n.contains("timeddemocharge") || n.contains("timeddemo") {
        return Some((10_000, 10_000));
    }
    if n.contains("radiationfield") || n.contains("radiationpool") {
        return Some((30_000, 30_000));
    }
    None
}

/// Template peels for common LifetimeUpdate users (msec).
/// Midpoint of INI Min/Max when they differ.
pub fn lifetime_msec_for_template(name: &str) -> Option<u32> {
    lifetime_msec_range_for_template(name).map(|(lo, hi)| lo.saturating_add(hi) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetime_expires() {
        let l = HostLifetimeUpdateData::from_delay_frames(10, 5);
        assert!(!l.tick(14));
        assert!(l.tick(15));
        assert_eq!(
            lifetime_msec_for_template("PoisonFieldMedium"),
            Some(30_000)
        );
    }

    #[test]
    fn lifetime_ini_min_max_random_and_hulk() {
        // C++ LifetimeUpdate.cpp:59-64 GameLogicRandomValue(min,max); delay < 1 → 1.
        assert_eq!(
            lifetime_msec_range_for_template("PoisonFieldMedium"),
            Some((30_000, 30_000))
        );
        let a = HostLifetimeUpdateData::from_ini_range(0, 1_000, 2_000, 11, false);
        let b = HostLifetimeUpdateData::from_ini_range(0, 1_000, 2_000, 11, false);
        assert_eq!(a.expire_at_frame, b.expire_at_frame);
        assert!(a.expire_at_frame >= 30 && a.expire_at_frame <= 60);

        set_hulk_max_lifetime_override(42);
        let (lo, hi) = effective_lifetime_msec_range(30_000, 30_000, true);
        assert_eq!(lo, hi);
        set_hulk_max_lifetime_override(-1);
        assert_eq!(
            effective_lifetime_msec_range(10_000, 20_000, true),
            (10_000, 20_000)
        );
    }
}
