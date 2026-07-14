//! Host America Sentry Drone residual (auto-detect + gun upgrade auto-fire).
//!
//! Residual slice (playability):
//! - Sentry Drone is always a stealth detector residual (`StealthDetectorUpdate`
//!   DetectionRange = **225**, DetectionRate = **900**ms) from spawn.
//! - `Upgrade_AmericaSentryDroneGun` research equips PRIMARY `SentryDroneGun`
//!   (retail WeaponSet PLAYER_UPGRADE residual): PrimaryDamage **8**, range **150**,
//!   DelayBetweenShots **200**ms → 6 frames.
//! - With gun equipped, idle Sentry auto-acquires and fires at nearby enemies
//!   (`DeployStyleAIUpdate AutoAcquireEnemiesWhenIdle = Yes` residual).
//! - DeployStyleAIUpdate pack/unpack residual: PackTime/UnpackTime **1000**ms →
//!   **30** frames each (honesty constants; full pack state machine fail-closed).
//! - StealthUpdate residual: StealthDelay **2000**ms → **60** frames re-cloak,
//!   forbidden FIRING_PRIMARY + MOVING residual names.
//!
//! Fail-closed honesty:
//! - Not full DeployStyleAIUpdate pack/unpack state machine /
//!   TurretsFunctionOnlyWhenDeployed animation matrix
//! - Not full StealthUpdate opacity / OrderIdleEnemies reveal path
//! - Not full IR detector FX / ExtraRequiredKindOf filters
//! - Not network detector / upgrade replication (network deferred)

/// Logic frames per second residual (C++ LOGICFRAMES_PER_SECOND).
pub const SENTRY_LOGIC_FPS: f32 = 30.0;

/// Retail Upgrade_AmericaSentryDroneGun name.
pub const UPGRADE_AMERICA_SENTRY_DRONE_GUN: &str = "Upgrade_AmericaSentryDroneGun";

/// Retail SentryDroneGun primary weapon template name.
pub const SENTRY_DRONE_GUN_WEAPON: &str = "SentryDroneGun";

/// Retail StealthDetectorUpdate DetectionRange residual.
pub const SENTRY_DETECTION_RANGE: f32 = 225.0;

/// Retail StealthDetectorUpdate DetectionRate residual (msec).
pub const SENTRY_DETECTION_RATE_MS: u32 = 900;
/// DetectionRate 900ms → 27 frames @ 30 FPS.
pub const SENTRY_DETECTION_RATE_FRAMES: u32 = 27;

/// Retail SentryDroneGun PrimaryDamage.
pub const SENTRY_GUN_DAMAGE: f32 = 8.0;
/// Retail SentryDroneGun PrimaryDamageRadius (intended-only).
pub const SENTRY_GUN_PRIMARY_RADIUS: f32 = 0.0;
/// Retail SentryDroneGun AttackRange.
pub const SENTRY_GUN_RANGE: f32 = 150.0;
/// Retail SentryDroneGun DelayBetweenShots residual (msec).
pub const SENTRY_GUN_DELAY_MS: u32 = 200;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const SENTRY_GUN_DELAY_FRAMES: u32 = 6;
/// Retail SentryDroneGun WeaponSpeed residual (dist/sec).
pub const SENTRY_GUN_WEAPON_SPEED: f32 = 600.0;

/// Residual audio event name.
pub const SENTRY_GUN_AUDIO: &str = "SentryDroneWeapon";

// --- DeployStyleAIUpdate residual (AmericaVehicleSentryDrone ModuleTag_04) ---

/// Retail DeployStyleAIUpdate PackTime residual (msec).
pub const SENTRY_PACK_TIME_MS: u32 = 1000;
/// PackTime 1000ms → 30 frames @ 30 FPS.
pub const SENTRY_PACK_TIME_FRAMES: u32 = 30;
/// Retail DeployStyleAIUpdate UnpackTime residual (msec).
pub const SENTRY_UNPACK_TIME_MS: u32 = 1000;
/// UnpackTime 1000ms → 30 frames @ 30 FPS.
pub const SENTRY_UNPACK_TIME_FRAMES: u32 = 30;
/// Retail TurretsFunctionOnlyWhenDeployed residual.
pub const SENTRY_TURRETS_ONLY_WHEN_DEPLOYED: bool = true;
/// Retail TurretsMustCenterBeforePacking residual.
pub const SENTRY_TURRETS_MUST_CENTER_BEFORE_PACK: bool = true;
/// Retail AutoAcquireEnemiesWhenIdle residual.
pub const SENTRY_AUTO_ACQUIRE_WHEN_IDLE: bool = true;

// --- StealthUpdate residual (AmericaVehicleSentryDrone ModuleTag_06) ---

/// Retail StealthUpdate StealthDelay residual (msec) — re-cloak delay.
pub const SENTRY_STEALTH_DELAY_MS: u32 = 2000;
/// StealthDelay 2000ms → 60 frames @ 30 FPS.
pub const SENTRY_STEALTH_DELAY_FRAMES: u32 = 60;
/// Retail StealthForbiddenConditions residual name list.
pub const SENTRY_STEALTH_FORBIDDEN_CONDITIONS: &[&str] = &["FIRING_PRIMARY", "MOVING"];
/// Retail InnateStealth residual.
pub const SENTRY_INNATE_STEALTH: bool = true;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn sentry_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / SENTRY_LOGIC_FPS)).round() as u32
}

/// Wave 49 residual honesty: detection / gun / pack-unpack / re-cloak constants.
pub fn honesty_sentry_drone_residual_ok() -> bool {
    (SENTRY_DETECTION_RANGE - 225.0).abs() < 0.01
        && SENTRY_DETECTION_RATE_MS == 900
        && SENTRY_DETECTION_RATE_FRAMES == sentry_ms_to_frames(SENTRY_DETECTION_RATE_MS)
        && (SENTRY_GUN_DAMAGE - 8.0).abs() < 0.01
        && (SENTRY_GUN_RANGE - 150.0).abs() < 0.01
        && SENTRY_GUN_DELAY_MS == 200
        && SENTRY_GUN_DELAY_FRAMES == sentry_ms_to_frames(SENTRY_GUN_DELAY_MS)
        && (SENTRY_GUN_PRIMARY_RADIUS - 0.0).abs() < 0.01
        && (SENTRY_GUN_WEAPON_SPEED - 600.0).abs() < 0.01
        && SENTRY_PACK_TIME_MS == 1000
        && SENTRY_PACK_TIME_FRAMES == sentry_ms_to_frames(SENTRY_PACK_TIME_MS)
        && SENTRY_UNPACK_TIME_MS == 1000
        && SENTRY_UNPACK_TIME_FRAMES == sentry_ms_to_frames(SENTRY_UNPACK_TIME_MS)
        && SENTRY_STEALTH_DELAY_MS == 2000
        && SENTRY_STEALTH_DELAY_FRAMES == sentry_ms_to_frames(SENTRY_STEALTH_DELAY_MS)
        && SENTRY_STEALTH_FORBIDDEN_CONDITIONS.len() == 2
        && SENTRY_STEALTH_FORBIDDEN_CONDITIONS[0] == "FIRING_PRIMARY"
        && SENTRY_STEALTH_FORBIDDEN_CONDITIONS[1] == "MOVING"
        && SENTRY_TURRETS_ONLY_WHEN_DEPLOYED
        && SENTRY_TURRETS_MUST_CENTER_BEFORE_PACK
        && SENTRY_AUTO_ACQUIRE_WHEN_IDLE
        && SENTRY_INNATE_STEALTH
        && SENTRY_DRONE_GUN_WEAPON == "SentryDroneGun"
        && UPGRADE_AMERICA_SENTRY_DRONE_GUN == "Upgrade_AmericaSentryDroneGun"
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_sentry_drone_residual_pack_ok() -> bool {
    honesty_sentry_drone_residual_ok()
}


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

    /// Wave 49: DetectionRange / gun residual / pack-unpack / re-cloak honesty.
    #[test]
    fn sentry_drone_residual_pack_honesty() {
        assert!(honesty_sentry_drone_residual_ok());
        assert_eq!(SENTRY_DETECTION_RANGE as i32, 225);
        assert_eq!(sentry_ms_to_frames(900), 27);
        assert_eq!(sentry_ms_to_frames(1000), 30);
        assert_eq!(sentry_ms_to_frames(2000), 60);
        assert_eq!(sentry_ms_to_frames(200), 6);
        assert_eq!(SENTRY_PACK_TIME_FRAMES, 30);
        assert_eq!(SENTRY_UNPACK_TIME_FRAMES, 30);
        assert_eq!(SENTRY_STEALTH_DELAY_FRAMES, 60);
        assert_eq!(SENTRY_DETECTION_RATE_FRAMES, 27);
        assert!((SENTRY_GUN_DAMAGE - 8.0).abs() < 0.01);
        assert!((SENTRY_GUN_RANGE - 150.0).abs() < 0.01);
        assert_eq!(SENTRY_GUN_DELAY_FRAMES, 6);
        assert_eq!(
            sentry_detection_range("AmericaVehicleSentryDrone"),
            Some(225.0)
        );
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn sentry_drone_residual_pack_honesty_wave71() {
        assert!(honesty_sentry_drone_residual_pack_ok());
        assert_eq!(SENTRY_DETECTION_RATE_FRAMES, 27);
        assert_eq!(SENTRY_GUN_DELAY_FRAMES, 6);
        assert_eq!(SENTRY_STEALTH_DELAY_FRAMES, 60);
    }

}
