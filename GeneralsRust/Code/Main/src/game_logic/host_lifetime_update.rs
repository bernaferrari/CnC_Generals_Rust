//! Host LifetimeUpdate residual (auto-die after min/max frames).
//!
//! C++: `LifetimeUpdate` sleeps until random frame in [min,max], then kills object
//! (which may trigger CreateObjectDie / FXListDie / etc.).

use serde::{Deserialize, Serialize};

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

    /// Returns true when lifetime expired this frame.
    pub fn tick(&self, current_frame: u32) -> bool {
        self.active && current_frame >= self.expire_at_frame && self.expire_at_frame > 0
    }
}

/// Template peels for common LifetimeUpdate users (msec).
pub fn lifetime_msec_for_template(name: &str) -> Option<u32> {
    let n = name.to_ascii_lowercase();
    if n.contains("sneakattack") && n.contains("start") {
        return Some(5_000);
    }
    if n.contains("poisonfieldmedium") {
        return Some(30_000);
    }
    if n.contains("poisonfieldsmall") {
        return Some(20_000);
    }
    if n.contains("poisonfieldlarge") {
        return Some(40_000);
    }
    if n.contains("firestorm") {
        return Some(6_000);
    }
    if n.contains("tntsticky") || (n.contains("sticky") && n.contains("bomb")) {
        return Some(10_000);
    }
    if n.contains("timeddemocharge") || n.contains("timeddemo") {
        return Some(10_000);
    }
    if n.contains("radiationfield") || n.contains("radiationpool") {
        return Some(30_000);
    }
    None
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
}
