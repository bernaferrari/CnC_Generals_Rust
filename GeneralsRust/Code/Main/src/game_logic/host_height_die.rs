//! Host HeightDieUpdate residual (die when altitude reaches target).
//!
//! C++: `HeightDieUpdate::update` kills when height-above-terrain ≤ TargetHeight
//! (optionally only while descending, after InitialDelay).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHeightDieData {
    pub target_height_above_terrain: f32,
    pub only_when_descending: bool,
    pub earliest_death_frame: u32,
    pub last_height: f32,
    pub has_died: bool,
    pub active: bool,
}

impl Default for HostHeightDieData {
    fn default() -> Self {
        Self {
            target_height_above_terrain: 0.0,
            only_when_descending: true,
            earliest_death_frame: 0,
            last_height: f32::MAX,
            has_died: false,
            active: true,
        }
    }
}

impl HostHeightDieData {
    pub fn with_target(height: f32, only_descending: bool, earliest_frame: u32) -> Self {
        Self {
            target_height_above_terrain: height.max(0.0),
            only_when_descending: only_descending,
            earliest_death_frame: earliest_frame,
            last_height: f32::MAX,
            has_died: false,
            active: true,
        }
    }

    /// Returns true when object should die this frame.
    /// `height_above_terrain` is world Y - terrain Y (host Y-up).
    pub fn tick(&mut self, current_frame: u32, height_above_terrain: f32, contained: bool) -> bool {
        if !self.active || self.has_died {
            return false;
        }
        if contained {
            self.last_height = height_above_terrain;
            return false;
        }
        if current_frame < self.earliest_death_frame {
            self.last_height = height_above_terrain;
            return false;
        }
        let mut direction_ok = true;
        if self.only_when_descending && height_above_terrain >= self.last_height {
            direction_ok = false;
        }
        self.last_height = height_above_terrain;
        if direction_ok && height_above_terrain <= self.target_height_above_terrain {
            self.has_died = true;
            return true;
        }
        false
    }
}

/// Common peels (target height, only_descending, initial delay msec).
pub fn height_die_config_for_template(name: &str) -> Option<(f32, bool, u32)> {
    let n = name.to_ascii_lowercase();
    if n.contains("aurorabomb") || n.contains("daisy") && n.contains("cutter") {
        return Some((10.0, true, 0));
    }
    if n.contains("scud") && n.contains("missile") {
        return Some((10.0, true, 0));
    }
    if n.contains("nuke") && n.contains("missile") {
        return Some((50.0, true, 1000)); // InitialDelay peel residual
    }
    if n.contains("carpetbomb") || n.contains("moab") {
        return Some((10.0, true, 0));
    }
    if n.contains("fuelair") || n.contains("gasbomb") {
        return Some((5.0, true, 0));
    }
    if n.contains("projectile") || n.contains("shell") && n.contains("artillery") {
        return Some((1.0, true, 0));
    }
    // Generic bomb/missile residual
    if n.contains("bomb") && !n.contains("bomber") {
        return Some((5.0, true, 0));
    }
    if n.contains("missile") && !n.contains("defender") && !n.contains("stinger") {
        return Some((10.0, true, 0));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_die_on_descent() {
        let mut h = HostHeightDieData::with_target(10.0, true, 0);
        assert!(!h.tick(1, 50.0, false));
        assert!(!h.tick(2, 55.0, false)); // ascending
        assert!(!h.tick(3, 20.0, false)); // descending but above
        assert!(h.tick(4, 8.0, false)); // below target while descending
        assert!(!h.tick(5, 0.0, false)); // already died
    }
}
