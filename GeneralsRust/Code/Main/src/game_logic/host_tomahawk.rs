//! Host America Tomahawk Launcher residual (long-range dual-radius missile).
//!
//! Residual slice (playability):
//! - `AmericaVehicleTomahawk` / `USA_Tomahawk` / variants spawn with PRIMARY
//!   `TomahawkMissileWeapon` (PrimaryDamage **150** / radius **10** +
//!   SecondaryDamage **50** / radius **25**, AttackRange **350**,
//!   MinimumAttackRange **100**, ClipReload **7000**ms → 210 frames).
//! - Fire residual: dual-radius splash (intended + primary/secondary rings).
//! - PreAttackDelay **250**ms residual honesty (recorded; not full PER_SHOT anim lock).
//!
//! Fail-closed honesty:
//! - Not full TomahawkMissile projectile lob / CapableOfFollowingWaypoints path
//! - Not full PreAttackDelay PER_SHOT anim / hide-show missile bone matrix
//! - Not Scout/Battle/Hellfire drone payload residual (see host_slave_drones)
//! - Not network tomahawk replication (network deferred)

use super::Weapon;

/// Retail primary weapon.
pub const TOMAHAWK_MISSILE_WEAPON: &str = "TomahawkMissileWeapon";

/// Retail PrimaryDamage.
pub const TOMAHAWK_PRIMARY_DAMAGE: f32 = 150.0;
/// Retail PrimaryDamageRadius.
pub const TOMAHAWK_PRIMARY_RADIUS: f32 = 10.0;
/// Retail SecondaryDamage.
pub const TOMAHAWK_SECONDARY_DAMAGE: f32 = 50.0;
/// Retail SecondaryDamageRadius.
pub const TOMAHAWK_SECONDARY_RADIUS: f32 = 25.0;
/// Retail AttackRange.
pub const TOMAHAWK_RANGE: f32 = 350.0;
/// Retail MinimumAttackRange.
pub const TOMAHAWK_MIN_RANGE: f32 = 100.0;
/// Retail ClipReloadTime 7000ms → 210 frames @ 30 FPS.
pub const TOMAHAWK_RELOAD_FRAMES: u32 = 210;
/// Retail PreAttackDelay 250ms → 8 frames @ 30 FPS (honesty residual).
pub const TOMAHAWK_PRE_ATTACK_FRAMES: u32 = 8;
/// Residual projectile speed (host hits still residual-instant).
pub const TOMAHAWK_PROJECTILE_SPEED: f32 = 200.0;

/// Residual fire audio.
pub const TOMAHAWK_FIRE_AUDIO: &str = "TomahawkWeapon";

/// Whether template is a residual Tomahawk launcher vehicle.
///
/// Fail-closed: name residual. Excludes missiles/projectiles/hulks.
pub fn is_tomahawk_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "usa_tomahawk"
        || n == "usa_tomahawklauncher"
        || n == "testtomahawk"
        || n == "americavehicletomahawk"
    {
        return true;
    }
    // Exclude missiles, weapons, hulks, locomotor, exhaust, crates.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("exhaust")
        || n.contains("detonation")
        || n.ends_with("missile")
        || n.contains("missileweapon")
    {
        return false;
    }
    // Living vehicle residual: *VehicleTomahawk* / *Tomahawk* chassis names.
    n.contains("vehicletomahawk")
        || (n.contains("tomahawk") && !n.contains("missile"))
}

/// Whether residual fire should apply Tomahawk residual path.
pub fn should_apply_tomahawk_residual(is_tomahawk: bool) -> bool {
    is_tomahawk
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Tomahawk primary Weapon.
pub fn tomahawk_weapon() -> Weapon {
    Weapon {
        damage: TOMAHAWK_PRIMARY_DAMAGE,
        range: TOMAHAWK_RANGE,
        min_range: TOMAHAWK_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(TOMAHAWK_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: TOMAHAWK_PROJECTILE_SPEED,
        pre_attack_delay: delay_frames_to_reload_secs(TOMAHAWK_PRE_ATTACK_FRAMES),
    }
}

/// Dual-radius residual damage at distance from impact (max of rings).
///
/// Intended target at impact takes PrimaryDamage; nearby units within
/// PrimaryDamageRadius take PrimaryDamage; SecondaryDamageRadius takes
/// SecondaryDamage residual.
pub fn tomahawk_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= TOMAHAWK_PRIMARY_RADIUS {
        TOMAHAWK_PRIMARY_DAMAGE
    } else if distance_from_impact <= TOMAHAWK_SECONDARY_RADIUS {
        TOMAHAWK_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target.
///
/// Retail RadiusDamageAffects includes SELF + ALLIES (friendly fire residual).
/// Host residual still skips self-source and under-construction; allies are
/// damageable (artillery friendly-fire residual honesty).
pub fn is_legal_tomahawk_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tomahawk_name_matrix() {
        assert!(is_tomahawk_template("AmericaVehicleTomahawk"));
        assert!(is_tomahawk_template("USA_Tomahawk"));
        assert!(is_tomahawk_template("USA_TomahawkLauncher"));
        assert!(is_tomahawk_template("TestTomahawk"));
        assert!(is_tomahawk_template("SupW_AmericaVehicleTomahawk"));
        assert!(!is_tomahawk_template("TomahawkMissile"));
        assert!(!is_tomahawk_template("TomahawkMissileWeapon"));
        assert!(!is_tomahawk_template("TomahawkMissileLocomotor"));
        assert!(!is_tomahawk_template("AmericaVehicleTomahawkHulk"));
        assert!(!is_tomahawk_template("USA_Crusader"));
        assert!(!is_tomahawk_template("USA_Ranger"));
    }

    #[test]
    fn weapon_and_dual_radius() {
        let w = tomahawk_weapon();
        assert!((w.damage - 150.0).abs() < 0.01);
        assert!((w.range - 350.0).abs() < 0.01);
        assert!((w.min_range - 100.0).abs() < 0.01);
        assert!((w.reload_time - 7.0).abs() < 0.05);

        assert!((tomahawk_damage_at(0.0) - 150.0).abs() < 0.01);
        assert!((tomahawk_damage_at(10.0) - 150.0).abs() < 0.01);
        assert!((tomahawk_damage_at(15.0) - 50.0).abs() < 0.01);
        assert!((tomahawk_damage_at(30.0)).abs() < 0.01);
    }
}
