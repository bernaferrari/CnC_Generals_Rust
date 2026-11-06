//! Host HighlanderBody residual.
//!
//! C++: `HighlanderBody::attemptDamage` clamps non-`DAMAGE_UNRESISTABLE` hits so
//! health never drops below 1. Unresistable (script kill / empty-hulk penalty /
//! explicit ::kill) still destroys the object.
//!
//! Installed only from the authored Body module (ThingTemplate::highlander_body).

use serde::{Deserialize, Serialize};

/// Honesty counter for Highlander clamps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHighlanderBodyRegistry {
    pub clamps: u32,
    pub unresistable_kills: u32,
}

impl HostHighlanderBodyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_clamp(&mut self) {
        self.clamps = self.clamps.saturating_add(1);
    }

    pub fn record_unresistable_kill(&mut self) {
        self.unresistable_kills = self.unresistable_kills.saturating_add(1);
    }

    pub fn honesty_clamp_ok(&self) -> bool {
        self.clamps > 0
    }
}

/// C++ HighlanderBody.cpp:33-34: clamp raw `DamageInfo.in.m_amount`
/// (`min(amount, getHealth()-1)`) BEFORE ActiveBody applies armor.
pub fn highlander_clamp_damage(
    current_health: f32,
    raw_amount: f32,
    unresistable: bool,
) -> (f32, bool) {
    if unresistable || current_health <= 0.0 {
        return (raw_amount, false);
    }
    if raw_amount >= current_health {
        let clamped = (current_health - 1.0).max(0.0);
        return (clamped, true);
    }
    (raw_amount, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_lethal_but_allows_unresistable() {
        let (d, c) = highlander_clamp_damage(50.0, 999.0, false);
        assert!(c);
        assert!((d - 49.0).abs() < 0.01);
        let (d2, c2) = highlander_clamp_damage(50.0, 999.0, true);
        assert!(!c2);
        assert!((d2 - 999.0).abs() < 0.01);
        let (d3, c3) = highlander_clamp_damage(50.0, 10.0, false);
        assert!(!c3);
        assert!((d3 - 10.0).abs() < 0.01);
    }
}
