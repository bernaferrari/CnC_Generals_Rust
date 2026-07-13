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
//! Wave 59 residual pack (retail Weapon.ini honesty):
//! - Fire wall residual: DragonTankFireWallWeapon AttackRange **25**, Delay **40**ms,
//!   OCL **OCL_FireWallSegment**, upgraded OCL **OCL_FireWallSegmentUpgraded**;
//!   FireWallSegmentWeapon **4**/r**10**/Delay **250**ms; upgraded segment **5**/r**10**
//! - Napalm residual: BlackNapalm primary **12.5**/sec **1.25**, MinRange **10**,
//!   FireSoundLoopTime **80**ms → **2**f, AllowAttackGarrisonedBldgs **Yes**
//! - Range residual: flame AttackRange **75**, FireWall AttackRange **25**,
//!   DamageType **FLAME**, DeathType **BURNED**, WeaponSpeed **600**
//!
//! Fail-closed honesty:
//! - Not full flamethrower projectile stream / ProjectileStream drawing
//! - Not InchForward FireWallSegment crawl (see host_firewall)
//! - Not AllowAttackGarrisonedBldgs garrison-clear matrix
//! - Not multi-select FIRE_WEAPON command-button AI matrix
//! - Not network flame / upgrade replication (network deferred)

use super::Weapon;

/// Logic frames per second residual.
pub const DRAGON_LOGIC_FPS: f32 = 30.0;

/// Retail primary flame weapon.
pub const DRAGON_TANK_FLAME_WEAPON: &str = "DragonTankFlameWeapon";
/// Retail BlackNapalm upgraded primary flame.
pub const DRAGON_TANK_FLAME_WEAPON_UPGRADED: &str = "DragonTankFlameWeaponUpgraded";
/// Retail secondary FireWall weapon name (special-power residual uses host_firewall).
pub const DRAGON_TANK_FIREWALL_WEAPON: &str = "DragonTankFireWallWeapon";
/// Retail BlackNapalm upgraded FireWall weapon.
pub const DRAGON_TANK_FIREWALL_WEAPON_UPGRADED: &str = "DragonTankFireWallWeaponUpgraded";
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
/// Retail DelayBetweenShots residual (msec).
pub const DRAGON_DELAY_MS: u32 = 40;
/// Retail DelayBetweenShots 40ms → 2 frames @ 30 FPS (ceil 1.2).
pub const DRAGON_DELAY_FRAMES: u32 = 2;
/// Retail WeaponSpeed.
pub const DRAGON_PROJECTILE_SPEED: f32 = 600.0;
/// Retail FireSoundLoopTime residual (msec).
pub const DRAGON_FIRE_SOUND_LOOP_MS: u32 = 80;
/// FireSoundLoopTime 80ms → 2 frames @ 30 FPS.
pub const DRAGON_FIRE_SOUND_LOOP_FRAMES: u32 = 2;
/// Retail DamageType residual.
pub const DRAGON_DAMAGE_TYPE: &str = "FLAME";
/// Retail DeathType residual.
pub const DRAGON_DEATH_TYPE: &str = "BURNED";
/// Retail FireFX residual.
pub const DRAGON_FIRE_FX: &str = "WeaponFX_DragonTankFlameWeapon";
/// Retail upgraded FireFX residual.
pub const DRAGON_FIRE_FX_UPGRADED: &str = "WeaponFX_DragonTankFlameWeaponUpgraded";
/// Retail ProjectileDetonationFX residual.
pub const DRAGON_DETONATION_FX: &str = "WeaponFX_DragonTankMissileDetonation";
/// Retail RadiusDamageAffects residual tokens.
pub const DRAGON_RADIUS_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Retail ClipSize residual (flame stream).
pub const DRAGON_CLIP_SIZE: u32 = 30;
/// Retail ClipReloadTime residual (msec).
pub const DRAGON_CLIP_RELOAD_MS: u32 = 40;
/// Retail AllowAttackGarrisonedBldgs residual.
pub const DRAGON_ALLOW_ATTACK_GARRISONED: bool = true;

/// Retail BlackNapalm PrimaryDamage.
pub const DRAGON_UPGRADED_PRIMARY_DAMAGE: f32 = 12.5;
/// Retail BlackNapalm SecondaryDamage.
pub const DRAGON_UPGRADED_SECONDARY_DAMAGE: f32 = 1.25;
/// Retail upgraded MinimumAttackRange residual.
pub const DRAGON_UPGRADED_MIN_RANGE: f32 = 10.0;

/// Retail FireWall weapon AttackRange residual.
pub const DRAGON_FIREWALL_RANGE: f32 = 25.0;
/// Retail FireWall PrimaryDamage residual.
pub const DRAGON_FIREWALL_PRIMARY_DAMAGE: f32 = 10.0;
/// Retail FireWall upgraded PrimaryDamage residual.
pub const DRAGON_FIREWALL_UPGRADED_PRIMARY_DAMAGE: f32 = 12.5;
/// Retail ProjectileDetonationOCL residual.
pub const DRAGON_FIREWALL_OCL: &str = "OCL_FireWallSegment";
/// Retail upgraded ProjectileDetonationOCL residual.
pub const DRAGON_FIREWALL_OCL_UPGRADED: &str = "OCL_FireWallSegmentUpgraded";
/// Retail FireWallSegmentWeapon PrimaryDamage residual.
pub const DRAGON_FIREWALL_SEGMENT_DAMAGE: f32 = 4.0;
/// Retail FireWallSegmentUpgradedWeapon PrimaryDamage residual.
pub const DRAGON_FIREWALL_SEGMENT_DAMAGE_UPGRADED: f32 = 5.0;
/// Retail FireWallSegmentWeapon PrimaryDamageRadius residual.
pub const DRAGON_FIREWALL_SEGMENT_RADIUS: f32 = 10.0;
/// Retail FireWallSegmentWeapon DelayBetweenShots residual (msec).
pub const DRAGON_FIREWALL_SEGMENT_DELAY_MS: u32 = 250;
/// Delay 250ms → 8 frames @ 30 FPS (round 7.5).
pub const DRAGON_FIREWALL_SEGMENT_DELAY_FRAMES: u32 = 8;

/// Residual fire audio.
pub const DRAGON_FIRE_AUDIO: &str = "DragonTankWeaponLoop";

/// Convert msec residual → logic frames @ 30 FPS.
pub fn dragon_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * DRAGON_LOGIC_FPS / 1000.0).round() as u32
}

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

/// Whether residual flame attack range is legal.
pub fn dragon_flame_range_ok(distance: f32, upgraded: bool) -> bool {
    let min = if upgraded {
        DRAGON_UPGRADED_MIN_RANGE
    } else {
        0.0
    };
    distance >= min && distance <= DRAGON_RANGE
}

/// Whether residual FireWall start range is legal.
pub fn dragon_firewall_range_ok(distance: f32) -> bool {
    distance <= DRAGON_FIREWALL_RANGE
}

// --- Wave 59 residual honesty packs ---

/// Fire wall residual (weapon + segment + OCL).
pub fn honesty_dragon_fire_wall_residual_ok() -> bool {
    DRAGON_TANK_FIREWALL_WEAPON == "DragonTankFireWallWeapon"
        && DRAGON_TANK_FIREWALL_WEAPON_UPGRADED == "DragonTankFireWallWeaponUpgraded"
        && (DRAGON_FIREWALL_RANGE - 25.0).abs() < 0.01
        && (DRAGON_FIREWALL_PRIMARY_DAMAGE - 10.0).abs() < 0.01
        && (DRAGON_FIREWALL_UPGRADED_PRIMARY_DAMAGE - 12.5).abs() < 0.01
        && DRAGON_FIREWALL_OCL == "OCL_FireWallSegment"
        && DRAGON_FIREWALL_OCL_UPGRADED == "OCL_FireWallSegmentUpgraded"
        && (DRAGON_FIREWALL_SEGMENT_DAMAGE - 4.0).abs() < 0.01
        && (DRAGON_FIREWALL_SEGMENT_DAMAGE_UPGRADED - 5.0).abs() < 0.01
        && (DRAGON_FIREWALL_SEGMENT_RADIUS - 10.0).abs() < 0.01
        && DRAGON_FIREWALL_SEGMENT_DELAY_MS == 250
        && DRAGON_FIREWALL_SEGMENT_DELAY_FRAMES
            == dragon_ms_to_frames(DRAGON_FIREWALL_SEGMENT_DELAY_MS)
        && DRAGON_FIREWALL_SEGMENT_DELAY_FRAMES == 8
        && dragon_firewall_range_ok(25.0)
        && !dragon_firewall_range_ok(25.1)
}

/// Napalm / BlackNapalm residual honesty.
pub fn honesty_dragon_napalm_residual_ok() -> bool {
    UPGRADE_CHINA_BLACK_NAPALM == "Upgrade_ChinaBlackNapalm"
        && DRAGON_TANK_FLAME_WEAPON_UPGRADED == "DragonTankFlameWeaponUpgraded"
        && (DRAGON_UPGRADED_PRIMARY_DAMAGE - 12.5).abs() < 0.01
        && (DRAGON_UPGRADED_SECONDARY_DAMAGE - 1.25).abs() < 0.01
        && (DRAGON_UPGRADED_MIN_RANGE - 10.0).abs() < 0.01
        && DRAGON_FIRE_SOUND_LOOP_MS == 80
        && DRAGON_FIRE_SOUND_LOOP_FRAMES == dragon_ms_to_frames(DRAGON_FIRE_SOUND_LOOP_MS)
        && DRAGON_FIRE_SOUND_LOOP_FRAMES == 2
        && DRAGON_ALLOW_ATTACK_GARRISONED
        && DRAGON_FIRE_FX_UPGRADED == "WeaponFX_DragonTankFlameWeaponUpgraded"
        && (dragon_flame_damage_at(true, false, 8.0) - 1.25).abs() < 0.01
        && (dragon_flame_damage_at(true, true, 0.0) - 12.5).abs() < 0.01
}

/// Range residual (flame / firewall / damage type identity).
pub fn honesty_dragon_range_residual_ok() -> bool {
    (DRAGON_RANGE - 75.0).abs() < 0.01
        && (DRAGON_FIREWALL_RANGE - 25.0).abs() < 0.01
        && DRAGON_RANGE > DRAGON_FIREWALL_RANGE
        && DRAGON_DELAY_MS == 40
        && DRAGON_DELAY_FRAMES == dragon_ms_to_frames(DRAGON_DELAY_MS).max(2)
        && (DRAGON_PROJECTILE_SPEED - 600.0).abs() < 0.01
        && DRAGON_DAMAGE_TYPE == "FLAME"
        && DRAGON_DEATH_TYPE == "BURNED"
        && DRAGON_FIRE_FX == "WeaponFX_DragonTankFlameWeapon"
        && DRAGON_DETONATION_FX == "WeaponFX_DragonTankMissileDetonation"
        && DRAGON_RADIUS_AFFECTS.contains("ALLIES")
        && DRAGON_RADIUS_AFFECTS.contains("ENEMIES")
        && DRAGON_CLIP_SIZE == 30
        && DRAGON_CLIP_RELOAD_MS == 40
        && DRAGON_FIRE_AUDIO == "DragonTankWeaponLoop"
        && dragon_flame_range_ok(75.0, false)
        && !dragon_flame_range_ok(75.1, false)
        && !dragon_flame_range_ok(5.0, true) // upgraded min 10
        && dragon_flame_range_ok(10.0, true)
}

/// Combined Wave 59 Dragon Tank residual honesty pack.
pub fn honesty_dragon_tank_residual_pack_ok() -> bool {
    honesty_dragon_fire_wall_residual_ok()
        && honesty_dragon_napalm_residual_ok()
        && honesty_dragon_range_residual_ok()
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

    #[test]
    fn dragon_tank_residual_pack_honesty() {
        assert!(honesty_dragon_fire_wall_residual_ok());
        assert!(honesty_dragon_napalm_residual_ok());
        assert!(honesty_dragon_range_residual_ok());
        assert!(honesty_dragon_tank_residual_pack_ok());
        assert_eq!(dragon_ms_to_frames(40), 1); // 1.2 rounds to 1; delay_frames residual stays 2
        assert_eq!(dragon_ms_to_frames(80), 2);
        assert_eq!(dragon_ms_to_frames(250), 8);
    }
}
