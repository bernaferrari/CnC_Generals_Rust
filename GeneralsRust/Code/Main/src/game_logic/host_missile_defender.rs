//! Host USA Missile Defender residual (missile primary + laser guided secondary).
//!
//! Residual slice (playability):
//! - `AmericaInfantryMissileDefender` / USA_ / SupW_ variants spawn with PRIMARY
//!   `MissileDefenderMissileWeapon` (dmg **40** / radius **5** / range **175** /
//!   Delay **1000**ms → 30 frames / AA+ground) and SECONDARY
//!   `MissileDefenderLaserGuidedMissileWeapon` (dmg **40** / radius **5** /
//!   range **300** / Delay **500**ms → 15 frames / AA+ground).
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//! - Laser guided special residual (`SpecialAbilityMissileDefenderLaserGuidedMissiles`):
//!   lock secondary weapon slot + attack target (StartAbilityRange **200** residual).
//!   SpecialPower ReloadTime **0** residual (no host cooldown gate).
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate PreparationTime 1000 / PersistentPrepTime 500 /
//!   LaserBeam special object attach-bone matrix
//! - Not full ScatterRadiusVsInfantry / projectile exhaust FX matrix
//! - Not PLAYER_UPGRADE DAMAGE 125% weapon-bonus residual
//! - Not network laser-lock replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Retail primary weapon.
pub const MISSILE_DEFENDER_MISSILE_WEAPON: &str = "MissileDefenderMissileWeapon";
/// Retail secondary laser guided weapon.
pub const MISSILE_DEFENDER_LASER_GUIDED_WEAPON: &str = "MissileDefenderLaserGuidedMissileWeapon";
/// Retail special power template.
pub const SPECIAL_ABILITY_MISSILE_DEFENDER_LASER: &str =
    "SpecialAbilityMissileDefenderLaserGuidedMissiles";

/// Retail PrimaryDamage (both weapons).
pub const MISSILE_DEFENDER_DAMAGE: f32 = 40.0;
/// Retail PrimaryDamageRadius residual splash.
pub const MISSILE_DEFENDER_SPLASH_RADIUS: f32 = 5.0;
/// Retail primary AttackRange.
pub const MISSILE_DEFENDER_PRIMARY_RANGE: f32 = 175.0;
/// Retail secondary (laser guided) AttackRange.
pub const MISSILE_DEFENDER_LASER_RANGE: f32 = 300.0;
/// Retail primary DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES: u32 = 30;
/// Retail secondary DelayBetweenShots 500ms → 15 frames @ 30 FPS.
pub const MISSILE_DEFENDER_LASER_DELAY_FRAMES: u32 = 15;
/// Retail WeaponSpeed residual (missile flight residual; host hits residual-instant).
pub const MISSILE_DEFENDER_PROJECTILE_SPEED: f32 = 600.0;
/// SpecialAbilityUpdate StartAbilityRange residual.
pub const LASER_GUIDED_START_ABILITY_RANGE: f32 = 200.0;

/// Residual fire audio.
pub const MISSILE_DEFENDER_FIRE_AUDIO: &str = "MissileDefenderWeapon";
/// Residual laser special initiate voice.
pub const LASER_GUIDED_INITIATE_AUDIO: &str = "MissileDefenderVoiceAttackLaser";

/// Whether template is a residual USA Missile Defender infantry.
///
/// Fail-closed: name residual. Excludes weapons/projectiles/locomotor tokens.
pub fn is_missile_defender_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("exhaust")
        || n.ends_with("missile") // MissileDefenderMissile projectile object
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testmissiledefender"
        || n == "usa_missiledefender"
        || n == "usa_missile_defender"
    {
        return true;
    }
    n.contains("missiledefender") || n.contains("missile_defender")
}

/// Build residual primary MissileDefenderMissileWeapon.
pub fn missile_defender_primary_weapon() -> Weapon {
    Weapon {
        damage: MISSILE_DEFENDER_DAMAGE,
        range: MISSILE_DEFENDER_PRIMARY_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: MISSILE_DEFENDER_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Build residual secondary MissileDefenderLaserGuidedMissileWeapon.
pub fn missile_defender_laser_guided_weapon() -> Weapon {
    Weapon {
        damage: MISSILE_DEFENDER_DAMAGE,
        range: MISSILE_DEFENDER_LASER_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(MISSILE_DEFENDER_LASER_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: MISSILE_DEFENDER_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for slot.
///
/// Slot 0 = primary, slot 1 = laser guided secondary.
pub fn missile_defender_weapon_stats(slot: u8) -> (f32, f32, u32, f32, f32) {
    if slot == 1 {
        (
            MISSILE_DEFENDER_DAMAGE,
            MISSILE_DEFENDER_LASER_RANGE,
            MISSILE_DEFENDER_LASER_DELAY_FRAMES,
            MISSILE_DEFENDER_SPLASH_RADIUS,
            MISSILE_DEFENDER_PROJECTILE_SPEED,
        )
    } else {
        (
            MISSILE_DEFENDER_DAMAGE,
            MISSILE_DEFENDER_PRIMARY_RANGE,
            MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES,
            MISSILE_DEFENDER_SPLASH_RADIUS,
            MISSILE_DEFENDER_PROJECTILE_SPEED,
        )
    }
}

/// Splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn missile_defender_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= MISSILE_DEFENDER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_missile_defender_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Missile Defender residual path.
pub fn should_apply_missile_defender_residual(is_missile_defender: bool) -> bool {
    is_missile_defender
}

/// Whether unit can issue laser guided special residual.
pub fn can_activate_laser_guided(is_missile_defender: bool, is_alive: bool) -> bool {
    is_missile_defender && is_alive
}

/// Whether target is within StartAbilityRange residual for laser guided special.
pub fn laser_guided_in_start_range(distance: f32) -> bool {
    distance <= LASER_GUIDED_START_ABILITY_RANGE
}

/// Whether residual fire is laser-guided secondary path (active_weapon_slot == 1).
pub fn is_laser_guided_slot(active_weapon_slot: u8) -> bool {
    active_weapon_slot == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missile_defender_name_matrix() {
        assert!(is_missile_defender_template("AmericaInfantryMissileDefender"));
        assert!(is_missile_defender_template("USA_MissileDefender"));
        assert!(is_missile_defender_template("SupW_AmericaInfantryMissileDefender"));
        assert!(is_missile_defender_template("TestMissileDefender"));
        assert!(!is_missile_defender_template("MissileDefenderMissileWeapon"));
        assert!(!is_missile_defender_template("MissileDefenderLaserGuidedMissileWeapon"));
        assert!(!is_missile_defender_template("MissileDefenderMissile"));
        assert!(!is_missile_defender_template("MissileDefenderLocomotor"));
        assert!(!is_missile_defender_template("AmericaInfantryRanger"));
        assert!(!is_missile_defender_template("ChinaInfantryTankHunter"));
        assert!(!is_missile_defender_template("GLAInfantryTunnelDefender"));
    }

    #[test]
    fn primary_and_laser_stats() {
        let (d, r, f, s, sp) = missile_defender_weapon_stats(0);
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 175.0).abs() < 0.01);
        assert_eq!(f, 30);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 600.0).abs() < 0.01);
        let w = missile_defender_primary_weapon();
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(w.can_target_air && w.can_target_ground);

        let (d2, r2, f2, _, _) = missile_defender_weapon_stats(1);
        assert!((d2 - 40.0).abs() < 0.01);
        assert!((r2 - 300.0).abs() < 0.01);
        assert_eq!(f2, 15);
        let lw = missile_defender_laser_guided_weapon();
        assert!((lw.range - 300.0).abs() < 0.01);
        assert!((lw.reload_time - 0.5).abs() < 0.01);
        assert!(lw.can_target_air && lw.can_target_ground);
    }

    #[test]
    fn splash_residual() {
        assert!((missile_defender_splash_damage_at(true, 100.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 4.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 5.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 5.1, 40.0)).abs() < 0.01);
    }

    #[test]
    fn laser_special_gate() {
        assert!(can_activate_laser_guided(true, true));
        assert!(!can_activate_laser_guided(false, true));
        assert!(!can_activate_laser_guided(true, false));
        assert!(laser_guided_in_start_range(200.0));
        assert!(!laser_guided_in_start_range(200.1));
        assert!(is_laser_guided_slot(1));
        assert!(!is_laser_guided_slot(0));
        assert!(should_apply_missile_defender_residual(true));
        assert!(!should_apply_missile_defender_residual(false));
    }
}
