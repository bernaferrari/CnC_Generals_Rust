//! Host America Pathfinder residual (innate stealth + stealth detector + sniper).
//!
//! Residual slice (playability):
//! - Pathfinder is always a stealth detector residual (`StealthDetectorUpdate`;
//!   DetectionRange unset → VisionRange = **200**).
//! - Innate stealth (`StealthUpdate InnateStealth = Yes`) from spawn.
//! - Stays stealthed while attacking (`StealthForbiddenConditions = MOVING` only;
//!   `stealth_breaks_on_attack = false`).
//! - Uncloaks while moving; re-cloaks immediately when stopped (StealthDelay = 0).
//! - PRIMARY `USAPathfinderSniperRifle` (100 dmg / 300 range / 2000 ms).
//!
//! Fail-closed honesty:
//! - Not full StealthUpdate pulse / FriendlyOpacity / OrderIdleEnemiesToAttackMe
//! - Not full IR detector FX / CanDetectWhileGarrisoned matrix
//! - Not full SCIENCE_Pathfinder prereq gate beyond residual spawn
//! - Not network detector / stealth replication (network deferred)

/// Retail primary weapon template name.
pub const PATHFINDER_SNIPER_WEAPON: &str = "USAPathfinderSniperRifle";

/// Retail VisionRange (used as DetectionRange when unset).
pub const PATHFINDER_DETECTION_RANGE: f32 = 200.0;

/// Retail sniper PrimaryDamage.
pub const PATHFINDER_SNIPER_DAMAGE: f32 = 100.0;
/// Retail sniper AttackRange.
pub const PATHFINDER_SNIPER_RANGE: f32 = 300.0;
/// Retail DelayBetweenShots 2000 ms → 60 frames @ 30 FPS.
pub const PATHFINDER_SNIPER_DELAY_FRAMES: u32 = 60;

/// Residual audio event name.
pub const PATHFINDER_WEAPON_AUDIO: &str = "PathfinderWeapon";

/// Whether template is a residual Pathfinder infantry.
///
/// Fail-closed: name residual (not full SCIENCE_Pathfinder prereq graph).
pub fn is_pathfinder_template(template_name: &str) -> bool {
    let n = template_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    if n.is_empty() {
        return false;
    }
    // Weapon / upgrade / science tokens are not the living unit.
    if n.contains("weapon")
        || n.contains("sniper")
        || n.contains("rifle")
        || n.starts_with("upgrade")
        || n.starts_with("science")
        || n.contains("command")
    {
        return false;
    }
    n.contains("pathfinder") || n == "usapathfinder"
}

/// Whether residual spawn should install detector + innate stealth fields.
pub fn pathfinder_spawn_is_detector(template_name: &str) -> bool {
    is_pathfinder_template(template_name)
}

/// Detection range residual for Pathfinder (retail VisionRange = 200).
pub fn pathfinder_detection_range(template_name: &str) -> Option<f32> {
    if is_pathfinder_template(template_name) {
        Some(PATHFINDER_DETECTION_RANGE)
    } else {
        None
    }
}

/// Maintain Pathfinder move-forbidden stealth residual.
///
/// Returns `(should_be_stealthed, changed)` for honesty bookkeeping when cloak
/// state flips due to MOVING / stop.
pub fn pathfinder_stealth_desired(
    is_pathfinder: bool,
    innate_stealth: bool,
    stealth_breaks_on_move: bool,
    is_alive: bool,
    is_moving_state: bool,
) -> Option<bool> {
    if !is_pathfinder || !innate_stealth || !is_alive {
        return None;
    }
    if stealth_breaks_on_move && is_moving_state {
        Some(false)
    } else {
        Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathfinder_name_matrix() {
        assert!(is_pathfinder_template("AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("USA_Pathfinder"));
        assert!(is_pathfinder_template("AirF_AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("SupW_AmericaInfantryPathfinder"));
        assert!(is_pathfinder_template("TestPathfinder"));
        assert!(!is_pathfinder_template("USA_Ranger"));
        assert!(!is_pathfinder_template("USAPathfinderSniperRifle"));
        assert!(!is_pathfinder_template("SciencePathfinder"));
        assert!(!is_pathfinder_template("AmericaVehicleSentryDrone"));
    }

    #[test]
    fn pathfinder_detect_and_stealth_desired() {
        assert!(pathfinder_spawn_is_detector("AmericaInfantryPathfinder"));
        assert_eq!(
            pathfinder_detection_range("AmericaInfantryPathfinder"),
            Some(PATHFINDER_DETECTION_RANGE)
        );
        assert_eq!(
            pathfinder_stealth_desired(true, true, true, true, true),
            Some(false),
            "moving pathfinder uncloaks"
        );
        assert_eq!(
            pathfinder_stealth_desired(true, true, true, true, false),
            Some(true),
            "idle pathfinder re-cloaks"
        );
        assert_eq!(
            pathfinder_stealth_desired(false, true, true, true, false),
            None
        );
    }
}
