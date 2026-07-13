//! Host America Sentry Drone residual (auto-detect + gun upgrade auto-fire).
//!
//! Residual slice (playability):
//! - Sentry Drone is always a stealth detector residual (`StealthDetectorUpdate`
//!   DetectionRange = 225) from spawn.
//! - `Upgrade_AmericaSentryDroneGun` research equips PRIMARY `SentryDroneGun`
//!   (retail WeaponSet PLAYER_UPGRADE residual).
//! - With gun equipped, idle Sentry auto-acquires and fires at nearby enemies
//!   (`DeployStyleAIUpdate AutoAcquireEnemiesWhenIdle = Yes` residual).
//!
//! Fail-closed honesty:
//! - Not full DeployStyleAIUpdate pack/unpack / turret-only-when-deployed matrix
//! - Not full StealthUpdate re-cloak delay / FIRING_PRIMARY break path beyond
//!   existing host stealth_breaks_on_attack residual
//! - Not full IR detector FX / ExtraRequiredKindOf filters
//! - Not network detector / upgrade replication (network deferred)

/// Retail Upgrade_AmericaSentryDroneGun name.
pub const UPGRADE_AMERICA_SENTRY_DRONE_GUN: &str = "Upgrade_AmericaSentryDroneGun";

/// Retail SentryDroneGun primary weapon template name.
pub const SENTRY_DRONE_GUN_WEAPON: &str = "SentryDroneGun";

/// Retail StealthDetectorUpdate DetectionRange residual.
pub const SENTRY_DETECTION_RANGE: f32 = 225.0;

/// Retail SentryDroneGun PrimaryDamage.
pub const SENTRY_GUN_DAMAGE: f32 = 8.0;
/// Retail SentryDroneGun AttackRange.
pub const SENTRY_GUN_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const SENTRY_GUN_DELAY_FRAMES: u32 = 6;

/// Residual audio event name.
pub const SENTRY_GUN_AUDIO: &str = "SentryDroneWeapon";

/// Whether template is a residual Sentry Drone.
///
/// Fail-closed: name residual (not full DeployStyleAIUpdate / DRONE kind matrix).
/// Excludes hulk debris / death OCL / weapon-token names that are not the living drone.
pub fn is_sentry_drone_template(template_name: &str) -> bool {
    let n = template_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    if n.is_empty() {
        return false;
    }
    // Death / hulk / debris residual objects are not living drones.
    if n.contains("hulk") || n.contains("die") || n.contains("debris") {
        return false;
    }
    // Weapon / upgrade tokens (SentryDroneGun, Upgrade_AmericaSentryDroneGun).
    if n.contains("gun") || n.contains("weapon") || n.starts_with("upgrade") {
        return false;
    }
    n.contains("sentrydrone") || n == "usasentrydrone"
}

/// Whether residual spawn should install detector fields.
pub fn sentry_spawn_is_detector(template_name: &str) -> bool {
    is_sentry_drone_template(template_name)
}

/// Detection range residual for Sentry (retail DetectionRange = 225).
pub fn sentry_detection_range(template_name: &str) -> Option<f32> {
    if is_sentry_drone_template(template_name) {
        Some(SENTRY_DETECTION_RANGE)
    } else {
        None
    }
}

/// Whether residual auto-fire path may run (gun present + idle-ish AI).
pub fn sentry_auto_fire_eligible(
    is_sentry: bool,
    has_weapon: bool,
    is_alive: bool,
    can_attack: bool,
    idle_or_attacking: bool,
) -> bool {
    is_sentry && has_weapon && is_alive && can_attack && idle_or_attacking
}

/// Legal residual target for Sentry auto-fire.
pub fn is_legal_sentry_auto_fire_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
    effectively_stealthed_hidden: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_neutral
        && !under_construction
        && is_attackable_or_combat_kind
        && !effectively_stealthed_hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentry_name_matrix() {
        assert!(is_sentry_drone_template("AmericaVehicleSentryDrone"));
        assert!(is_sentry_drone_template("USA_SentryDrone"));
        assert!(is_sentry_drone_template("AirF_AmericaVehicleSentryDrone"));
        assert!(is_sentry_drone_template("SupW_AmericaVehicleSentryDrone"));
        assert!(is_sentry_drone_template("Lazr_AmericaVehicleSentryDrone"));
        assert!(is_sentry_drone_template("TestSentryDrone"));
        assert!(!is_sentry_drone_template("USA_Ranger"));
        assert!(!is_sentry_drone_template("AmericaVehicleSentryDroneHulk"));
        assert!(!is_sentry_drone_template("AmericaHumvee"));
        assert!(!is_sentry_drone_template("SentryDroneGun"));
    }

    #[test]
    fn detector_spawn_residual() {
        assert!(sentry_spawn_is_detector("AmericaVehicleSentryDrone"));
        assert_eq!(
            sentry_detection_range("AmericaVehicleSentryDrone"),
            Some(SENTRY_DETECTION_RANGE)
        );
        assert_eq!(sentry_detection_range("USA_Ranger"), None);
        assert!(!sentry_spawn_is_detector("USA_Ranger"));
    }

    #[test]
    fn auto_fire_eligibility() {
        assert!(sentry_auto_fire_eligible(true, true, true, true, true));
        assert!(!sentry_auto_fire_eligible(true, false, true, true, true));
        assert!(!sentry_auto_fire_eligible(false, true, true, true, true));
        assert!(!sentry_auto_fire_eligible(true, true, false, true, true));
        assert!(!sentry_auto_fire_eligible(true, true, true, false, true));
        assert!(!sentry_auto_fire_eligible(true, true, true, true, false));
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_sentry_auto_fire_target(
            true, false, false, false, true, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            false, false, false, false, true, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            true, true, false, false, true, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            true, false, true, false, true, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            true, false, false, true, true, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            true, false, false, false, false, false
        ));
        assert!(!is_legal_sentry_auto_fire_target(
            true, false, false, false, true, true
        ));
    }
}
