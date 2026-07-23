//! Host SquishCollide residual (infantry crushed under vehicle wheels).
//!
//! C++: `SquishCollide::onCollide` — when a crusher (other) overlaps this object
//! with crusherLevel > 0, non-ally, geom hit at 1.0 crush radius, and velocity
//! points toward victim → DAMAGE_CRUSH / DEATH_CRUSHED with HUGE_DAMAGE_AMOUNT.
//!
//! Residual playability slice:
//! - Velocity-toward-victim gate (dot product residual)
//! - Ally skip residual
//! - Hijacker / TNT-hunter goal skip residual (template peels)
//! - 1.0 crush radius residual for tight infantry hit
//!
//! Fail-closed: not full partition geomCollidesWithGeom matrix / hijacker module
//! pointer lookup beyond template peels.

use serde::{Deserialize, Serialize};

/// C++ HUGE_DAMAGE_AMOUNT residual for squish kill.
pub const SQUISH_HUGE_DAMAGE: f32 = 999_999.0;
/// C++ crush geometry radius residual (major/minor forced to 1.0).
pub const SQUISH_CRUSH_RADIUS: f32 = 1.0;

/// True if crusher velocity points toward victim (C++ to·vel > 0).
///
/// `crusher_pos` / `victim_pos` are XZ world; `vel` is crusher velocity XZ.
pub fn velocity_toward_victim(
    crusher_pos: (f32, f32),
    victim_pos: (f32, f32),
    vel: (f32, f32),
) -> bool {
    let to_x = victim_pos.0 - crusher_pos.0;
    let to_z = victim_pos.1 - crusher_pos.1;
    to_x * vel.0 + to_z * vel.1 > 0.0
}

/// Tight crush radius residual: victim selection radius clamped for hit test.
pub fn squish_hit_radius(selection_radius: f32) -> f32 {
    // C++ forces major/minor to 1.0 for the victim geom during collide test.
    let _ = selection_radius;
    SQUISH_CRUSH_RADIUS
}

/// Distance 2D within tight crush residual.
pub fn within_squish_radius(
    crusher_pos: (f32, f32),
    victim_pos: (f32, f32),
    crusher_radius: f32,
) -> bool {
    let dx = crusher_pos.0 - victim_pos.0;
    let dz = crusher_pos.1 - victim_pos.1;
    let dist = (dx * dx + dz * dz).sqrt();
    dist <= crusher_radius.max(1.0) + SQUISH_CRUSH_RADIUS
}

/// C++ goal-object skip residual: hijacker / TNT hunter on victim targeting crusher.
pub fn should_skip_squish_for_goal_ability(victim_template: &str) -> bool {
    let n = victim_template.to_ascii_lowercase();
    n.contains("hijacker") || n.contains("jarmen") || n.contains("tankhunter")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSquishCollideLog {
    pub squish_kills: u32,
    pub velocity_rejects: u32,
    pub ally_rejects: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toward_and_away() {
        // Crusher at 0 moving +X, victim at +5 → toward.
        assert!(velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (2.0, 0.0)));
        // Moving -X away from victim at +5.
        assert!(!velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (-2.0, 0.0)));
    }

    #[test]
    fn skip_hijacker_peels() {
        assert!(should_skip_squish_for_goal_ability("GLAInfantryHijacker"));
        assert!(!should_skip_squish_for_goal_ability("GLAInfantryRebel"));
    }
}
