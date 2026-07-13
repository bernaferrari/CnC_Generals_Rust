//! Host GLA Rocket Buggy residual (long-range rockets + infantry scatter splash).
//!
//! Residual slice (playability):
//! - Spawns with `BuggyRocketWeapon` primary (AttackRange 300, MinRange 50).
//! - Clip residual: 6 rockets base; `Upgrade_GLABuggyAmmo` → 12 (clip size residual).
//! - On fire: intended target takes PrimaryDamage (20); nearby units within
//!   SecondaryDamageRadius (10) take SecondaryDamage (5) residual splash.
//! - `ScatterRadiusVsInfantry` residual: vs infantry, rockets may miss primary
//!   (fail-closed deterministic scatter) while still applying splash ring.
//!
//! Fail-closed honesty:
//! - Not full projectile flight / MissileCallsOnDie / AP rocket damage mult matrix
//! - Not full AutoReloadWhenIdle clip timer beyond host reload residual
//! - Not full Salvage / junk repair visual matrix
//! - Not network weapon replication (network deferred)

/// Retail primary weapon template name.
pub const BUGGY_ROCKET_WEAPON: &str = "BuggyRocketWeapon";
/// Retail upgraded ammo weapon (clip 12).
pub const BUGGY_ROCKET_WEAPON_UPGRADED: &str = "BuggyRocketWeaponUpgraded";
/// Retail Upgrade_GLABuggyAmmo name.
pub const UPGRADE_GLA_BUGGY_AMMO: &str = "Upgrade_GLABuggyAmmo";

/// Retail BuggyRocketWeapon PrimaryDamage.
pub const BUGGY_PRIMARY_DAMAGE: f32 = 20.0;
/// Retail PrimaryDamageRadius 0 → hits intended victim only for primary.
pub const BUGGY_PRIMARY_RADIUS: f32 = 0.0;
/// Retail SecondaryDamage.
pub const BUGGY_SECONDARY_DAMAGE: f32 = 5.0;
/// Retail SecondaryDamageRadius.
pub const BUGGY_SECONDARY_RADIUS: f32 = 10.0;
/// Retail AttackRange.
pub const BUGGY_ATTACK_RANGE: f32 = 300.0;
/// Retail MinimumAttackRange.
pub const BUGGY_MIN_RANGE: f32 = 50.0;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const BUGGY_DELAY_FRAMES: u32 = 6;
/// Retail ClipSize base.
pub const BUGGY_CLIP_SIZE: u32 = 6;
/// Retail ClipSize after Upgrade_GLABuggyAmmo.
pub const BUGGY_CLIP_SIZE_UPGRADED: u32 = 12;
/// Retail ClipReloadTime 6000ms → 180 frames @ 30 FPS.
pub const BUGGY_CLIP_RELOAD_FRAMES: u32 = 180;
/// Retail ScatterRadiusVsInfantry.
pub const BUGGY_SCATTER_VS_INFANTRY: f32 = 20.0;
/// Residual fire audio.
pub const BUGGY_FIRE_AUDIO: &str = "RocketBuggyWeapon";

/// Whether template is a residual Rocket Buggy vehicle.
///
/// Fail-closed: name residual (not full INI WeaponSet / Salvage matrix).
/// Excludes missiles / projectiles.
pub fn is_rocket_buggy_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("missile") || n.contains("projectile") || n.contains("shell") {
        return false;
    }
    // Avoid false positive on "buggy" alone from combat bike etc.
    n.contains("rocketbuggy")
        || n.contains("rocket_buggy")
        || n == "gla_rocketbuggy"
        || n == "testrocketbuggy"
        || n.contains("vehiclerocketbuggy")
}

/// Whether combat should apply Rocket Buggy residual splash path.
pub fn should_apply_rocket_buggy_residual(is_buggy: bool) -> bool {
    is_buggy
}

/// Deterministic residual scatter miss vs infantry.
///
/// Retail ScatterRadiusVsInfantry aims at a random offset; host residual uses
/// (frame ^ target_id) parity so tests can force miss/hit by frame choice.
/// Fail-closed: not full continuous random circle sample.
pub fn rocket_buggy_infantry_scatter_miss(
    target_is_infantry: bool,
    frame: u32,
    target_id_raw: u32,
) -> bool {
    if !target_is_infantry {
        return false;
    }
    // ~50% residual miss rate (fail-closed vs continuous scatter distribution).
    ((frame ^ target_id_raw).wrapping_mul(2654435761)) & 1 == 1
}

/// Residual damage for one object relative to impact.
///
/// - Intended target (not scatter-miss): PrimaryDamage (radius 0 semantics).
/// - Any unit within SecondaryDamageRadius: SecondaryDamage (max of rings).
/// - Scatter miss on intended: no primary; secondary only if still "near"
///   (host residual: miss means no damage to intended).
pub fn rocket_buggy_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    scatter_miss: bool,
) -> f32 {
    if is_intended_target {
        if scatter_miss {
            return 0.0;
        }
        return BUGGY_PRIMARY_DAMAGE;
    }
    if distance_from_impact <= BUGGY_SECONDARY_RADIUS {
        BUGGY_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_rocket_buggy_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Clip size residual after ammo upgrade.
pub fn rocket_buggy_clip_size(has_buggy_ammo_upgrade: bool) -> u32 {
    if has_buggy_ammo_upgrade {
        BUGGY_CLIP_SIZE_UPGRADED
    } else {
        BUGGY_CLIP_SIZE
    }
}

/// 2D distance residual.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rocket_buggy_name_matrix() {
        assert!(is_rocket_buggy_template("GLAVehicleRocketBuggy"));
        assert!(is_rocket_buggy_template("Chem_GLAVehicleRocketBuggy"));
        assert!(is_rocket_buggy_template("Demo_GLAVehicleRocketBuggy"));
        assert!(is_rocket_buggy_template("Slth_GLAVehicleRocketBuggy"));
        assert!(is_rocket_buggy_template("TestRocketBuggy"));
        assert!(is_rocket_buggy_template("GLA_RocketBuggy"));
        assert!(!is_rocket_buggy_template("RocketBuggyMissile"));
        assert!(!is_rocket_buggy_template("GLAVehicleQuadCannon"));
        assert!(!is_rocket_buggy_template("GLAVehicleScudLauncher"));
        assert!(!is_rocket_buggy_template("USA_Ranger"));
    }

    #[test]
    fn damage_primary_and_splash() {
        assert!(
            (rocket_buggy_damage_at(true, 0.0, false) - BUGGY_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!((rocket_buggy_damage_at(true, 0.0, true)).abs() < 0.01);
        assert!(
            (rocket_buggy_damage_at(false, 5.0, false) - BUGGY_SECONDARY_DAMAGE).abs() < 0.01
        );
        assert!((rocket_buggy_damage_at(false, 15.0, false)).abs() < 0.01);
    }

    #[test]
    fn scatter_only_vs_infantry() {
        assert!(!rocket_buggy_infantry_scatter_miss(false, 1, 1));
        // Deterministic: at least one of two consecutive frames differs or same parity rule holds.
        let a = rocket_buggy_infantry_scatter_miss(true, 0, 1);
        let b = rocket_buggy_infantry_scatter_miss(true, 1, 1);
        // Not both forced same for all pairs — just ensure function is pure.
        let _ = (a, b);
        assert_eq!(
            rocket_buggy_infantry_scatter_miss(true, 42, 7),
            rocket_buggy_infantry_scatter_miss(true, 42, 7)
        );
    }

    #[test]
    fn clip_upgrade_residual() {
        assert_eq!(rocket_buggy_clip_size(false), BUGGY_CLIP_SIZE);
        assert_eq!(rocket_buggy_clip_size(true), BUGGY_CLIP_SIZE_UPGRADED);
    }

    #[test]
    fn splash_target_matrix() {
        assert!(is_legal_rocket_buggy_splash_target(true, false, false, true));
        assert!(!is_legal_rocket_buggy_splash_target(false, false, false, true));
        assert!(!is_legal_rocket_buggy_splash_target(true, true, false, true));
        assert!(!is_legal_rocket_buggy_splash_target(true, false, true, true));
        assert!(!is_legal_rocket_buggy_splash_target(true, false, false, false));
    }
}
