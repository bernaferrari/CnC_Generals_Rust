//! Host America Fire Base residual (howitzer dual-radius structure defense).
//!
//! Residual slice (playability):
//! - `AmericaFireBase` / AirF_/SupW_/Lazr_ variants spawn with PRIMARY
//!   `FireBaseHowitzerGun` (PrimaryDamage **75** / radius **10**, range **275**,
//!   min **50**, Delay **2000**ms → 60 frames).
//! - Fire residual: intended + PrimaryDamageRadius **10** splash.
//! - Structure residual: CAN_ATTACK base-defense howitzer (not full spawn matrix).
//!
//! Fail-closed honesty:
//! - Not full SPAWNS_ARE_THE_WEAPONS / garrison-howitzer HiveStructureBody matrix
//! - Not full Turret pitch / ScaleWeaponSpeed lob projectile matrix
//! - Not full ScatterRadiusVsInfantry residual miss cone
//! - Not network Fire Base replication (network deferred)

use super::Weapon;

/// Retail primary weapon.
pub const FIRE_BASE_HOWITZER_WEAPON: &str = "FireBaseHowitzerGun";

/// Retail PrimaryDamage.
pub const FIRE_BASE_DAMAGE: f32 = 75.0;
/// Retail PrimaryDamageRadius.
pub const FIRE_BASE_PRIMARY_RADIUS: f32 = 10.0;
/// Retail AttackRange.
pub const FIRE_BASE_RANGE: f32 = 275.0;
/// Retail MinimumAttackRange.
pub const FIRE_BASE_MIN_RANGE: f32 = 50.0;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const FIRE_BASE_DELAY_FRAMES: u32 = 60;
/// Residual projectile speed (ScaleWeaponSpeed lob residual honesty).
pub const FIRE_BASE_PROJECTILE_SPEED: f32 = 300.0;

/// Residual fire audio.
pub const FIRE_BASE_FIRE_AUDIO: &str = "StrategyCenter_ArtilleryRound";

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual America Fire Base structure.
///
/// Fail-closed: name residual. Excludes howitzer projectiles / armor tokens / hulks.
pub fn is_fire_base_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "americafirebase"
        || n == "usa_firebase"
        || n == "testfirebase"
        || n == "airfamericafirebase"
        || n == "supwamericafirebase"
        || n == "lazramericafirebase"
    {
        return true;
    }
    // Exclude non-structure residual objects.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.contains("armor")
        || n.contains("howitzer")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("explosion")
    {
        return false;
    }
    n.contains("firebase")
}

/// Whether residual fire should apply Fire Base residual path.
pub fn should_apply_fire_base_residual(is_fire_base: bool) -> bool {
    is_fire_base
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Fire Base howitzer Weapon.
pub fn fire_base_weapon() -> Weapon {
    Weapon {
        damage: FIRE_BASE_DAMAGE,
        range: FIRE_BASE_RANGE,
        min_range: FIRE_BASE_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(FIRE_BASE_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: FIRE_BASE_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Residual damage at distance from impact (intended / primary ring).
pub fn fire_base_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= FIRE_BASE_PRIMARY_RADIUS {
        FIRE_BASE_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash / fire target.
pub fn is_legal_fire_base_target(
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
    fn fire_base_name_matrix() {
        assert!(is_fire_base_template("AmericaFireBase"));
        assert!(is_fire_base_template("USA_FireBase"));
        assert!(is_fire_base_template("TestFireBase"));
        assert!(is_fire_base_template("AirF_AmericaFireBase"));
        assert!(is_fire_base_template("SupW_AmericaFireBase"));
        assert!(is_fire_base_template("Lazr_AmericaFireBase"));
        assert!(!is_fire_base_template("FireBaseHowitzerGun"));
        assert!(!is_fire_base_template("FireBaseArmor"));
        assert!(!is_fire_base_template("AmericaStrategyCenter"));
        assert!(!is_fire_base_template("AmericaPatriotBattery"));
    }

    #[test]
    fn weapon_and_radius() {
        let w = fire_base_weapon();
        assert!((w.damage - 75.0).abs() < 0.01);
        assert!((w.range - 275.0).abs() < 0.01);
        assert!((w.min_range - 50.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.05);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);

        assert!((fire_base_damage_at(0.0) - 75.0).abs() < 0.01);
        assert!((fire_base_damage_at(10.0) - 75.0).abs() < 0.01);
        assert!((fire_base_damage_at(15.0)).abs() < 0.01);
    }
}
