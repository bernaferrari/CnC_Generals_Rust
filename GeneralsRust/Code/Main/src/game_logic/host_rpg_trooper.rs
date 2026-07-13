//! Host GLA RPG Trooper / Tunnel Defender residual (rocket + AP Rockets upgrade).
//!
//! Residual slice (playability):
//! - `GLAInfantryTunnelDefender` / Chem_/Demo_/Slth_/GC_* variants spawn with PRIMARY
//!   `TunnelDefenderRocketWeapon` (dmg **40** / radius **5** / range **175** /
//!   min **5** / Delay **1000**ms → 30 frames / AA+ground).
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//! - AP Rockets PLAYER_UPGRADE residual (`Upgrade_GLAAPRockets`):
//!   WeaponBonus DAMAGE **125%** → PrimaryDamage **50**.
//!
//! Fail-closed honesty:
//! - Not full ScatterRadiusVsInfantry / projectile exhaust FX matrix
//! - Not full Salvager crate matrix
//! - Not network AP / RPG replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Retail primary weapon.
pub const TUNNEL_DEFENDER_ROCKET_WEAPON: &str = "TunnelDefenderRocketWeapon";
/// Retail Upgrade_GLAAPRockets.
pub const UPGRADE_GLA_AP_ROCKETS: &str = "Upgrade_GLAAPRockets";

/// Retail PrimaryDamage.
pub const RPG_TROOPER_DAMAGE: f32 = 40.0;
/// Retail PrimaryDamageRadius residual splash.
pub const RPG_TROOPER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange.
pub const RPG_TROOPER_RANGE: f32 = 175.0;
/// Retail MinimumAttackRange.
pub const RPG_TROOPER_MIN_RANGE: f32 = 5.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const RPG_TROOPER_BASE_DELAY_FRAMES: u32 = 30;
/// Retail WeaponSpeed residual (missile flight residual; host hits still residual-instant).
pub const RPG_TROOPER_PROJECTILE_SPEED: f32 = 600.0;

/// AP Rockets WeaponBonus DAMAGE 125%.
pub const RPG_AP_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const RPG_TROOPER_FIRE_AUDIO: &str = "RPGTrooperWeapon";

/// Whether template is a residual RPG Trooper / Tunnel Defender infantry.
///
/// Fail-closed: name residual. Excludes weapons/biker/debris tokens.
pub fn is_rpg_trooper_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("biker")
        || n.contains("site") // StingerSite
        || n.contains("network") // TunnelNetwork structure
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testrpgtrooper"
        || n == "testrpg"
        || n == "testtunneldefender"
        || n == "gla_rpgtrooper"
        || n == "gla_rpg"
    {
        return true;
    }
    n.contains("tunneldefender")
        || n.contains("tunnel_defender")
        || n.contains("rpgtrooper")
        || n.contains("rpg_trooper")
}

/// Whether upgrade set includes AP Rockets residual.
pub fn has_ap_rockets_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let n = u.to_ascii_lowercase();
        n.contains("aprockets")
            || n.contains("ap_rockets")
            || n == "upgrade_glaaprockets"
            || n.contains("glaaprockets")
    })
}

/// Apply AP Rockets residual damage mult when upgrade present.
pub fn rpg_trooper_damage_with_ap(has_ap_rockets: bool) -> f32 {
    if has_ap_rockets {
        RPG_TROOPER_DAMAGE * RPG_AP_DAMAGE_MULT
    } else {
        RPG_TROOPER_DAMAGE
    }
}

/// Delay frames residual.
pub fn rpg_trooper_delay_frames() -> u32 {
    RPG_TROOPER_BASE_DELAY_FRAMES
}

/// (damage, range, min_range, delay_frames, splash_radius, projectile_speed).
pub fn rpg_trooper_weapon_stats(has_ap_rockets: bool) -> (f32, f32, f32, u32, f32, f32) {
    (
        rpg_trooper_damage_with_ap(has_ap_rockets),
        RPG_TROOPER_RANGE,
        RPG_TROOPER_MIN_RANGE,
        rpg_trooper_delay_frames(),
        RPG_TROOPER_SPLASH_RADIUS,
        RPG_TROOPER_PROJECTILE_SPEED,
    )
}

/// Build residual RPG Weapon with optional AP Rockets residual.
pub fn rpg_trooper_weapon(has_ap_rockets: bool) -> Weapon {
    let (damage, range, min_range, delay, _splash, speed) =
        rpg_trooper_weapon_stats(has_ap_rockets);
    Weapon {
        damage,
        range,
        min_range,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: speed,
        pre_attack_delay: 0.0,
    }
}

/// Splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn rpg_trooper_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= RPG_TROOPER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_rpg_trooper_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply RPG Trooper residual path.
pub fn should_apply_rpg_trooper_residual(is_rpg_trooper: bool) -> bool {
    is_rpg_trooper
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn rpg_trooper_name_matrix() {
        assert!(is_rpg_trooper_template("GLAInfantryTunnelDefender"));
        assert!(is_rpg_trooper_template("GLA_TunnelDefender"));
        assert!(is_rpg_trooper_template("Demo_GLAInfantryTunnelDefender"));
        assert!(is_rpg_trooper_template("Chem_GLAInfantryTunnelDefender"));
        assert!(is_rpg_trooper_template("Slth_GLAInfantryTunnelDefender"));
        assert!(is_rpg_trooper_template("TestRPGTrooper"));
        assert!(is_rpg_trooper_template("GLA_RPG"));
        assert!(!is_rpg_trooper_template("TunnelDefenderRocketWeapon"));
        assert!(!is_rpg_trooper_template("TunnelDefenderBikerRocketWeapon"));
        assert!(!is_rpg_trooper_template("TunnelDefenderMissile"));
        assert!(!is_rpg_trooper_template("GLA_StingerSite"));
        assert!(!is_rpg_trooper_template("GLATunnelNetwork"));
        assert!(!is_rpg_trooper_template("Upgrade_GLAAPRockets"));
        assert!(!is_rpg_trooper_template("GLAInfantryRebel"));
        assert!(!is_rpg_trooper_template("ChinaInfantryTankHunter"));
        assert!(!is_rpg_trooper_template("MissileDefenderLocomotor"));
    }

    #[test]
    fn base_rpg_stats() {
        let (d, r, min_r, f, s, sp) = rpg_trooper_weapon_stats(false);
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 175.0).abs() < 0.01);
        assert!((min_r - 5.0).abs() < 0.01);
        assert_eq!(f, 30);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 600.0).abs() < 0.01);
        let w = rpg_trooper_weapon(false);
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.min_range - 5.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn ap_rockets_damage() {
        assert!((rpg_trooper_damage_with_ap(false) - 40.0).abs() < 0.01);
        assert!((rpg_trooper_damage_with_ap(true) - 50.0).abs() < 0.01);
        let w = rpg_trooper_weapon(true);
        assert!((w.damage - 50.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
    }

    #[test]
    fn splash_residual() {
        assert!((rpg_trooper_splash_damage_at(true, 100.0, 40.0) - 40.0).abs() < 0.01);
        assert!((rpg_trooper_splash_damage_at(false, 4.0, 40.0) - 40.0).abs() < 0.01);
        assert!((rpg_trooper_splash_damage_at(false, 5.0, 40.0) - 40.0).abs() < 0.01);
        assert!((rpg_trooper_splash_damage_at(false, 5.1, 40.0)).abs() < 0.01);
    }

    #[test]
    fn ap_upgrade_detect() {
        let mut tags = HashSet::new();
        assert!(!has_ap_rockets_upgrade(&tags));
        tags.insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        assert!(has_ap_rockets_upgrade(&tags));
    }

    #[test]
    fn residual_gate() {
        assert!(should_apply_rpg_trooper_residual(true));
        assert!(!should_apply_rpg_trooper_residual(false));
        assert!(is_legal_rpg_trooper_splash_target(true, false, false, true));
        assert!(!is_legal_rpg_trooper_splash_target(false, false, false, true));
        assert!(!is_legal_rpg_trooper_splash_target(true, true, false, true));
    }
}
