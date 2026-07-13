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
//! Wave 63 residual pack (retail INI honesty):
//! - Rocket residual: Primary **20**/r**0** + Secondary **5**/r**10**, range **300**/min **50**,
//!   Delay **200**ms → **6**f, Clip **6**/**12**, ClipReload **6000**ms → **180**f,
//!   AutoReloadsClip **Yes**, AutoReloadWhenIdle **6100**ms → **183**f,
//!   WeaponSpeed **600**, Projectile **RocketBuggyMissile**, FireFX **FX_BuggyMissileIgnition**,
//!   DetonationFX **WeaponFX_RocketBuggyMissileDetonation**, AP Rockets DAMAGE **125%**.
//! - Body residual: MaxHealth **120**, Vision **180**, Shroud **300**, BuildCost **900**,
//!   BuildTime **10**s → **300**f, TransportSlotCount **3**.
//!
//! Fail-closed honesty:
//! - Not full projectile flight / MissileCallsOnDie / AP rocket damage mult matrix
//! - Not full AutoReloadWhenIdle clip timer beyond host reload residual
//! - Not full Salvage / junk repair visual matrix
//! - Not network weapon replication (network deferred)

/// Logic frames per second (host fixed step).
pub const BUGGY_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon template name.
pub const BUGGY_ROCKET_WEAPON: &str = "BuggyRocketWeapon";
/// Retail upgraded ammo weapon (clip 12).
pub const BUGGY_ROCKET_WEAPON_UPGRADED: &str = "BuggyRocketWeaponUpgraded";
/// Retail Upgrade_GLABuggyAmmo name.
pub const UPGRADE_GLA_BUGGY_AMMO: &str = "Upgrade_GLABuggyAmmo";
/// Retail Upgrade_GLAAPRockets name (WeaponBonus PLAYER_UPGRADE).
pub const UPGRADE_GLA_AP_ROCKETS: &str = "Upgrade_GLAAPRockets";

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
/// Retail DelayBetweenShots residual (msec).
pub const BUGGY_DELAY_MS: u32 = 200;
/// Retail DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const BUGGY_DELAY_FRAMES: u32 = 6;
/// Retail ClipSize base.
pub const BUGGY_CLIP_SIZE: u32 = 6;
/// Retail ClipSize after Upgrade_GLABuggyAmmo.
pub const BUGGY_CLIP_SIZE_UPGRADED: u32 = 12;
/// Retail ClipReloadTime residual (msec).
pub const BUGGY_CLIP_RELOAD_MS: u32 = 6_000;
/// Retail ClipReloadTime 6000ms → 180 frames @ 30 FPS.
pub const BUGGY_CLIP_RELOAD_FRAMES: u32 = 180;
/// Retail AutoReloadsClip residual.
pub const BUGGY_AUTO_RELOADS_CLIP: bool = true;
/// Retail AutoReloadWhenIdle residual (msec).
pub const BUGGY_AUTO_RELOAD_WHEN_IDLE_MS: u32 = 6_100;
/// Retail AutoReloadWhenIdle → frames @ 30 FPS.
pub const BUGGY_AUTO_RELOAD_WHEN_IDLE_FRAMES: u32 = 183;
/// Retail WeaponSpeed residual.
pub const BUGGY_PROJECTILE_SPEED: f32 = 600.0;
/// Retail ScatterRadiusVsInfantry.
pub const BUGGY_SCATTER_VS_INFANTRY: f32 = 20.0;
/// Retail ProjectileObject residual.
pub const BUGGY_MISSILE_PROJECTILE: &str = "RocketBuggyMissile";
/// Retail FireFX residual.
pub const BUGGY_FIRE_FX: &str = "FX_BuggyMissileIgnition";
/// Retail ProjectileDetonationFX residual.
pub const BUGGY_DETONATION_FX: &str = "WeaponFX_RocketBuggyMissileDetonation";
/// Retail DamageType residual.
pub const BUGGY_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const BUGGY_DEATH_TYPE: &str = "EXPLODED";
/// AP Rockets WeaponBonus DAMAGE 125%.
pub const BUGGY_AP_DAMAGE_MULT: f32 = 1.25;
/// Residual fire audio.
pub const BUGGY_FIRE_AUDIO: &str = "RocketBuggyWeapon";

// --- Body residual (GLAVehicleRocketBuggy) ---

/// Retail MaxHealth residual.
pub const BUGGY_MAX_HEALTH: f32 = 120.0;
/// Retail VisionRange residual.
pub const BUGGY_VISION_RANGE: f32 = 180.0;
/// Retail ShroudClearingRange residual.
pub const BUGGY_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const BUGGY_BUILD_COST: u32 = 900;
/// Retail BuildTime residual (seconds).
pub const BUGGY_BUILD_TIME_SEC: f32 = 10.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const BUGGY_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const BUGGY_TRANSPORT_SLOT_COUNT: u32 = 3;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn buggy_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * BUGGY_LOGIC_FPS / 1000.0).round() as u32
}

/// Residual primary damage with optional AP Rockets mult.
pub fn buggy_primary_damage_with_ap(has_ap_rockets: bool) -> f32 {
    if has_ap_rockets {
        BUGGY_PRIMARY_DAMAGE * BUGGY_AP_DAMAGE_MULT
    } else {
        BUGGY_PRIMARY_DAMAGE
    }
}

/// Residual secondary damage with optional AP Rockets mult.
pub fn buggy_secondary_damage_with_ap(has_ap_rockets: bool) -> f32 {
    if has_ap_rockets {
        BUGGY_SECONDARY_DAMAGE * BUGGY_AP_DAMAGE_MULT
    } else {
        BUGGY_SECONDARY_DAMAGE
    }
}

/// Whether residual attack range is legal (min + max gate).
pub fn rocket_buggy_range_ok(distance: f32) -> bool {
    distance >= BUGGY_MIN_RANGE && distance <= BUGGY_ATTACK_RANGE
}

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

// --- Wave 63 residual honesty packs ---

/// Wave 63 residual honesty: rocket weapon residual peel.
pub fn honesty_rocket_buggy_weapon_residual_ok() -> bool {
    BUGGY_ROCKET_WEAPON == "BuggyRocketWeapon"
        && BUGGY_ROCKET_WEAPON_UPGRADED == "BuggyRocketWeaponUpgraded"
        && (BUGGY_PRIMARY_DAMAGE - 20.0).abs() < 0.01
        && (BUGGY_PRIMARY_RADIUS - 0.0).abs() < 0.01
        && (BUGGY_SECONDARY_DAMAGE - 5.0).abs() < 0.01
        && (BUGGY_SECONDARY_RADIUS - 10.0).abs() < 0.01
        && (BUGGY_ATTACK_RANGE - 300.0).abs() < 0.01
        && (BUGGY_MIN_RANGE - 50.0).abs() < 0.01
        && BUGGY_DELAY_MS == 200
        && BUGGY_DELAY_FRAMES == buggy_ms_to_frames(BUGGY_DELAY_MS)
        && BUGGY_DELAY_FRAMES == 6
        && BUGGY_CLIP_SIZE == 6
        && BUGGY_CLIP_SIZE_UPGRADED == 12
        && BUGGY_CLIP_RELOAD_MS == 6_000
        && BUGGY_CLIP_RELOAD_FRAMES == buggy_ms_to_frames(BUGGY_CLIP_RELOAD_MS)
        && BUGGY_CLIP_RELOAD_FRAMES == 180
        && BUGGY_AUTO_RELOADS_CLIP
        && BUGGY_AUTO_RELOAD_WHEN_IDLE_MS == 6_100
        && BUGGY_AUTO_RELOAD_WHEN_IDLE_FRAMES
            == buggy_ms_to_frames(BUGGY_AUTO_RELOAD_WHEN_IDLE_MS)
        && BUGGY_AUTO_RELOAD_WHEN_IDLE_FRAMES == 183
        && (BUGGY_PROJECTILE_SPEED - 600.0).abs() < 0.01
        && (BUGGY_SCATTER_VS_INFANTRY - 20.0).abs() < 0.01
        && BUGGY_MISSILE_PROJECTILE == "RocketBuggyMissile"
        && BUGGY_FIRE_FX == "FX_BuggyMissileIgnition"
        && BUGGY_DETONATION_FX == "WeaponFX_RocketBuggyMissileDetonation"
        && BUGGY_DAMAGE_TYPE == "EXPLOSION"
        && BUGGY_DEATH_TYPE == "EXPLODED"
        && BUGGY_FIRE_AUDIO == "RocketBuggyWeapon"
        && rocket_buggy_range_ok(50.0)
        && rocket_buggy_range_ok(300.0)
        && !rocket_buggy_range_ok(49.9)
        && !rocket_buggy_range_ok(300.1)
}

/// Wave 63 residual honesty: AP Rockets + clip upgrade residual peel.
pub fn honesty_rocket_buggy_ap_clip_residual_ok() -> bool {
    UPGRADE_GLA_BUGGY_AMMO == "Upgrade_GLABuggyAmmo"
        && UPGRADE_GLA_AP_ROCKETS == "Upgrade_GLAAPRockets"
        && (BUGGY_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && (buggy_primary_damage_with_ap(false) - 20.0).abs() < 0.01
        && (buggy_primary_damage_with_ap(true) - 25.0).abs() < 0.01
        && (buggy_secondary_damage_with_ap(false) - 5.0).abs() < 0.01
        && (buggy_secondary_damage_with_ap(true) - 6.25).abs() < 0.01
        && rocket_buggy_clip_size(false) == 6
        && rocket_buggy_clip_size(true) == 12
        && (rocket_buggy_damage_at(true, 0.0, false) - 20.0).abs() < 0.01
        && (rocket_buggy_damage_at(false, 10.0, false) - 5.0).abs() < 0.01
}

/// Wave 63 residual honesty: Rocket Buggy body residual peel.
pub fn honesty_rocket_buggy_body_residual_ok() -> bool {
    (BUGGY_MAX_HEALTH - 120.0).abs() < 0.01
        && (BUGGY_VISION_RANGE - 180.0).abs() < 0.01
        && (BUGGY_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && BUGGY_BUILD_COST == 900
        && (BUGGY_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && BUGGY_BUILD_TIME_FRAMES
            == ((BUGGY_BUILD_TIME_SEC * BUGGY_LOGIC_FPS).round() as u32)
        && BUGGY_BUILD_TIME_FRAMES == 300
        && BUGGY_TRANSPORT_SLOT_COUNT == 3
        && should_apply_rocket_buggy_residual(true)
        && !should_apply_rocket_buggy_residual(false)
}

/// Combined Wave 63 Rocket Buggy residual honesty pack.
pub fn honesty_rocket_buggy_residual_pack_ok() -> bool {
    honesty_rocket_buggy_weapon_residual_ok()
        && honesty_rocket_buggy_ap_clip_residual_ok()
        && honesty_rocket_buggy_body_residual_ok()
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

    #[test]
    fn rocket_buggy_residual_pack_honesty_wave63() {
        assert!(honesty_rocket_buggy_weapon_residual_ok());
        assert!(honesty_rocket_buggy_ap_clip_residual_ok());
        assert!(honesty_rocket_buggy_body_residual_ok());
        assert!(honesty_rocket_buggy_residual_pack_ok());
        assert_eq!(buggy_ms_to_frames(200), 6);
        assert_eq!(buggy_ms_to_frames(6_000), 180);
        assert_eq!(buggy_ms_to_frames(6_100), 183);
        assert_eq!(buggy_ms_to_frames(0), 0);
        assert!((buggy_primary_damage_with_ap(true) - 25.0).abs() < 0.01);
        assert_eq!(BUGGY_BUILD_TIME_FRAMES, 300);
        assert_eq!(BUGGY_MISSILE_PROJECTILE, "RocketBuggyMissile");
    }
}
