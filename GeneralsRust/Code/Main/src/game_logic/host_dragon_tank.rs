//! Host China Dragon Tank residual (primary flame stream + BlackNapalm upgrade).
//!
//! Residual slice (playability):
//! - Spawns with PRIMARY `DragonTankFlameWeapon` (dmg **10** / primary radius **5** /
//!   secondary dmg **1** / secondary radius **10** / range **75** / Delay **40**ms).
//! - Fire residual: intended + units in PrimaryDamageRadius take full primary;
//!   units in SecondaryDamageRadius take secondary residual (fail-closed falloff).
//! - BlackNapalm PLAYER_UPGRADE residual (`Upgrade_ChinaBlackNapalm`):
//!   `DragonTankFlameWeaponUpgraded` dmg **12.5** / sec **1.25** (same radii/range).
//! - FireWall / Firestorm secondary is already covered by `host_firewall` special-power
//!   residual (not re-implemented here).
//!
//! Fail-closed honesty:
//! - Not full flamethrower projectile stream / ProjectileStream drawing
//! - Not InchForward FireWallSegment crawl (see host_firewall)
//! - Not AllowAttackGarrisonedBldgs garrison-clear matrix
//! - Not multi-select FIRE_WEAPON command-button AI matrix
//! - Not network flame / upgrade replication (network deferred)

use super::Weapon;

/// Retail primary flame weapon.
pub const DRAGON_TANK_FLAME_WEAPON: &str = "DragonTankFlameWeapon";
/// Retail BlackNapalm upgraded primary flame.
pub const DRAGON_TANK_FLAME_WEAPON_UPGRADED: &str = "DragonTankFlameWeaponUpgraded";
/// Retail secondary FireWall weapon name (special-power residual uses host_firewall).
pub const DRAGON_TANK_FIREWALL_WEAPON: &str = "DragonTankFireWallWeapon";
/// Retail Upgrade_ChinaBlackNapalm.
pub const UPGRADE_CHINA_BLACK_NAPALM: &str = "Upgrade_ChinaBlackNapalm";

/// Retail PrimaryDamage base.
pub const DRAGON_PRIMARY_DAMAGE: f32 = 10.0;
/// Retail PrimaryDamageRadius.
pub const DRAGON_PRIMARY_RADIUS: f32 = 5.0;
/// Retail SecondaryDamage base.
pub const DRAGON_SECONDARY_DAMAGE: f32 = 1.0;
/// Retail SecondaryDamageRadius.
pub const DRAGON_SECONDARY_RADIUS: f32 = 10.0;
/// Retail AttackRange.
pub const DRAGON_RANGE: f32 = 75.0;
/// Retail DelayBetweenShots 40ms → 2 frames @ 30 FPS (ceil 1.2).
pub const DRAGON_DELAY_FRAMES: u32 = 2;
/// Retail WeaponSpeed.
pub const DRAGON_PROJECTILE_SPEED: f32 = 600.0;

/// Retail BlackNapalm PrimaryDamage.
pub const DRAGON_UPGRADED_PRIMARY_DAMAGE: f32 = 12.5;
/// Retail BlackNapalm SecondaryDamage.
pub const DRAGON_UPGRADED_SECONDARY_DAMAGE: f32 = 1.25;
/// Retail upgraded MinimumAttackRange residual.
pub const DRAGON_UPGRADED_MIN_RANGE: f32 = 10.0;

/// Residual fire audio.
pub const DRAGON_FIRE_AUDIO: &str = "DragonTankWeaponLoop";

/// Whether template is a residual Dragon Tank vehicle.
///
/// Fail-closed: name residual (not full BlackNapalm / W3D matrix).
/// Excludes weapons / projectiles / FireWallSegment / science tokens.
pub fn is_dragon_tank_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / projectile / segment / upgrade tokens are not the living vehicle.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("segment")
        || n.contains("shell")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("firewall")
        || n.contains("firestorm")
        || n.contains("dead")
        || n.contains("hulk")
        || n.contains("debris")
    {
        return false;
    }
    n.contains("dragontank")
        || n.contains("tankdragon")
        || n == "china_dragontank"
        || n == "testdragontank"
        || (n.contains("dragon") && (n.contains("tank") || n.contains("vehicle")))
}

/// Whether residual fire should apply Dragon flame residual path.
pub fn should_apply_dragon_flame_residual(is_dragon: bool) -> bool {
    is_dragon
}

/// Whether BlackNapalm upgrade is active on residual unit (tag present).
pub fn has_black_napalm_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("blacknapalm") || l == "upgrade_chinablacknapalm"
    })
}

/// (primary_dmg, secondary_dmg, range, min_range, delay_frames) for upgrade state.
pub fn dragon_flame_stats(upgraded: bool) -> (f32, f32, f32, f32, u32) {
    if upgraded {
        (
            DRAGON_UPGRADED_PRIMARY_DAMAGE,
            DRAGON_UPGRADED_SECONDARY_DAMAGE,
            DRAGON_RANGE,
            DRAGON_UPGRADED_MIN_RANGE,
            DRAGON_DELAY_FRAMES,
        )
    } else {
        (
            DRAGON_PRIMARY_DAMAGE,
            DRAGON_SECONDARY_DAMAGE,
            DRAGON_RANGE,
            0.0,
            DRAGON_DELAY_FRAMES,
        )
    }
}

/// Weapon template name for upgrade state.
pub fn dragon_flame_weapon_name(upgraded: bool) -> &'static str {
    if upgraded {
        DRAGON_TANK_FLAME_WEAPON_UPGRADED
    } else {
        DRAGON_TANK_FLAME_WEAPON
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual primary Weapon for upgrade state.
pub fn dragon_flame_weapon(upgraded: bool) -> Weapon {
    let (dmg, _sec, range, min_range, delay) = dragon_flame_stats(upgraded);
    Weapon {
        damage: dmg,
        range,
        min_range,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: DRAGON_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Flame residual damage at distance from impact.
///
/// - Intended target always takes full primary (even if slightly outside radius residual).
/// - Others within PrimaryDamageRadius take full primary.
/// - Others within SecondaryDamageRadius take secondary residual.
/// - Beyond secondary radius: 0.
pub fn dragon_flame_damage_at(
    upgraded: bool,
    is_intended_target: bool,
    distance_from_impact: f32,
) -> f32 {
    let (primary, secondary, _, _, _) = dragon_flame_stats(upgraded);
    if is_intended_target {
        return primary;
    }
    if distance_from_impact <= DRAGON_PRIMARY_RADIUS {
        primary
    } else if distance_from_impact <= DRAGON_SECONDARY_RADIUS {
        secondary
    } else {
        0.0
    }
}

/// Legal residual flame splash target.
pub fn is_legal_dragon_flame_target(
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
    use std::collections::HashSet;

    #[test]
    fn dragon_tank_name_matrix() {
        assert!(is_dragon_tank_template("ChinaTankDragon"));
        assert!(is_dragon_tank_template("China_DragonTank"));
        assert!(is_dragon_tank_template("Infa_ChinaTankDragon"));
        assert!(is_dragon_tank_template("Nuke_ChinaTankDragon"));
        assert!(is_dragon_tank_template("Tank_ChinaTankDragon"));
        assert!(is_dragon_tank_template("TestDragonTank"));
        assert!(!is_dragon_tank_template("DragonTankFlameWeapon"));
        assert!(!is_dragon_tank_template("DragonTankFireWallWeapon"));
        assert!(!is_dragon_tank_template("DragonTankFlameProjectile"));
        assert!(!is_dragon_tank_template("FireWallSegment"));
        assert!(!is_dragon_tank_template("Upgrade_ChinaBlackNapalm"));
        assert!(!is_dragon_tank_template("ChinaTankGattling"));
        assert!(!is_dragon_tank_template("USA_Ranger"));
        assert!(!is_dragon_tank_template("ChinaTankDragonDeadHull"));
    }

    #[test]
    fn flame_stats_and_splash() {
        let (p, s, r, min, d) = dragon_flame_stats(false);
        assert!((p - 10.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((r - 75.0).abs() < 0.01);
        assert!((min - 0.0).abs() < 0.01);
        assert_eq!(d, 2);

        let (pu, su, _, minu, _) = dragon_flame_stats(true);
        assert!((pu - 12.5).abs() < 0.01);
        assert!((su - 1.25).abs() < 0.01);
        assert!((minu - 10.0).abs() < 0.01);

        assert!((dragon_flame_damage_at(false, true, 0.0) - 10.0).abs() < 0.01);
        assert!((dragon_flame_damage_at(false, false, 3.0) - 10.0).abs() < 0.01);
        assert!((dragon_flame_damage_at(false, false, 8.0) - 1.0).abs() < 0.01);
        assert!((dragon_flame_damage_at(false, false, 15.0)).abs() < 0.01);
        assert!((dragon_flame_damage_at(true, false, 8.0) - 1.25).abs() < 0.01);

        let w = dragon_flame_weapon(false);
        assert!((w.damage - 10.0).abs() < 0.01);
        assert!((w.range - 75.0).abs() < 0.01);
        assert!((w.reload_time - (2.0 / 30.0)).abs() < 0.01);
    }

    #[test]
    fn black_napalm_tag() {
        let mut tags = HashSet::new();
        assert!(!has_black_napalm_upgrade(&tags));
        tags.insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        assert!(has_black_napalm_upgrade(&tags));
    }
}
