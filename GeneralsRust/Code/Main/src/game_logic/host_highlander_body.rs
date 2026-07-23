//! Host HighlanderBody residual.
//!
//! C++: `HighlanderBody::attemptDamage` clamps non-`DAMAGE_UNRESISTABLE` hits so
//! health never drops below 1. Unresistable (script kill / empty-hulk penalty /
//! explicit ::kill) still destroys the object.
//!
//! Retail peels: NatureProp trees/props, some WeaponObjects trail markers,
//! Airforce general markers. Template-name residual + explicit install flag.

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

/// True when template should install HighlanderBody residual.
pub fn is_highlander_body_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Explicit install / residual markers.
    if n.contains("highlander") {
        return true;
    }
    // NatureProp.ini trees / bushes (not vehicles that merely contain "tree" in
    // a longer unrelated token — require start or common prop prefixes).
    if n.starts_with("tree")
        || n.starts_with("bush")
        || n.starts_with("shrub")
        || n.contains("natureprop")
        || n.starts_with("alpinetree")
        || n.starts_with("oak")
        || n.starts_with("pine")
    {
        return true;
    }
    // Trail remnant markers that use HighlanderBody in retail WeaponObjects.
    n.contains("trailremnant")
}

/// C++ HighlanderBody clamp: non-unresistable lethal → leave 1 HP.
pub fn highlander_clamp_damage(
    current_health: f32,
    actual_damage: f32,
    unresistable: bool,
) -> (f32, bool) {
    if unresistable || current_health <= 0.0 {
        return (actual_damage, false);
    }
    if actual_damage >= current_health {
        let clamped = (current_health - 1.0).max(0.0);
        return (clamped, true);
    }
    (actual_damage, false)
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

    #[test]
    fn template_detection() {
        assert!(is_highlander_body_template("Tree01"));
        assert!(is_highlander_body_template("NaturePropRock"));
        assert!(is_highlander_body_template("AlpineTree02"));
        assert!(!is_highlander_body_template("AmericaTankCrusader"));
        assert!(!is_highlander_body_template("GLAVehicleBattleBus"));
    }
}
