//! Host China Overlord / Emperor main gun residual (dual-radius shell + Uranium).
//!
//! Residual slice (playability):
//! - `ChinaTankOverlord` / general variants + `Tank_ChinaTankEmperor` spawn with
//!   PRIMARY `OverlordTankGun` (PrimaryDamage **80** / radius **5** +
//!   SecondaryDamage **20** / radius **10**, AttackRange **175**,
//!   ClipReload **2000**ms → 60 frames; ClipSize **2** honesty).
//! - Fire residual: dual-radius splash (intended + primary/secondary rings).
//! - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
//!   WeaponBonus DAMAGE **125%** on primary + secondary rings (100 / 25).
//! - Portable gattling addon residual remains exclusive fire path when installed
//!   (host_overlord_addons); primary damage still reads weapon residual.
//!
//! Fail-closed honesty:
//! - Not full ClipSize=2 DelayBetweenShots 300ms dual-volley cadence matrix
//! - Not full ScatterRadiusVsInfantry / projectile shell lob / W3D bone matrix
//! - Not full Nuclear Tanks death weapon residual
//! - Not HelixMinigun residual (Helix uses separate residual path)
//! - Not network uranium / dual-radius replication (network deferred)

use super::Weapon;
use crate::game_logic::host_overlord_addons::{is_emperor_template, is_overlord_tank_template};
use std::collections::HashSet;

/// Retail primary weapon.
pub const OVERLORD_TANK_GUN: &str = "OverlordTankGun";
/// Retail Tank General variant (same residual numbers as base OverlordTankGun).
pub const TANK_OVERLORD_TANK_GUN: &str = "Tank_OverlordTankGun";
/// Retail Upgrade_ChinaUraniumShells (WeaponBonusUpgrade → PLAYER_UPGRADE).
pub const UPGRADE_CHINA_URANIUM_SHELLS: &str = "Upgrade_ChinaUraniumShells";

/// Retail PrimaryDamage base.
pub const OVERLORD_PRIMARY_DAMAGE: f32 = 80.0;
/// Retail PrimaryDamageRadius residual splash.
pub const OVERLORD_PRIMARY_RADIUS: f32 = 5.0;
/// Retail SecondaryDamage.
pub const OVERLORD_SECONDARY_DAMAGE: f32 = 20.0;
/// Retail SecondaryDamageRadius.
pub const OVERLORD_SECONDARY_RADIUS: f32 = 10.0;
/// Retail AttackRange.
pub const OVERLORD_RANGE: f32 = 175.0;
/// Retail ClipReloadTime 2000ms → 60 frames @ 30 FPS (sustained fire residual).
/// ClipSize=2 / DelayBetweenShots 300ms dual-volley cadence is honesty-only.
pub const OVERLORD_RELOAD_FRAMES: u32 = 60;
/// Retail ClipSize residual honesty.
pub const OVERLORD_CLIP_SIZE: u32 = 2;
/// Retail WeaponSpeed residual (shell flight residual; host hits residual-instant).
pub const OVERLORD_PROJECTILE_SPEED: f32 = 300.0;

/// Uranium PLAYER_UPGRADE WeaponBonus DAMAGE 125%.
pub const OVERLORD_URANIUM_DAMAGE_MULT: f32 = 1.25;

/// Residual fire audio.
pub const OVERLORD_FIRE_AUDIO: &str = "OverlordTankWeapon";

/// Whether template is a residual Overlord / Emperor chassis that fires OverlordTankGun.
///
/// Fail-closed: name residual. Excludes portable payloads, Helix, shells, debris.
pub fn is_overlord_gun_chassis(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("gattling")
        || n.contains("gatling")
        || n.contains("propaganda")
        || n.contains("bunker")
        || n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("command")
        || n.contains("helix")
        || n.contains("minigun")
        || n.ends_with("gun")
        || n.contains("tankgun")
        || n.contains("locomotor")
        || n.contains("exhaust")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testoverlord"
        || n == "testemperor"
        || n == "china_overlordtank"
        || n == "china_overlord"
        || n == "tank_chinatankemperor"
    {
        return true;
    }
    is_overlord_tank_template(template_name) || is_emperor_template(template_name)
}

/// Whether residual fire should apply Overlord dual-radius gun residual.
///
/// Callers should skip when portable gattling exclusive residual is active.
pub fn should_apply_overlord_gun_residual(is_chassis: bool, has_gattling_addon: bool) -> bool {
    is_chassis && !has_gattling_addon
}

/// Whether Uranium Shells upgrade tag is present.
pub fn has_uranium_shells_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("uraniumshell") || l == "upgrade_chinauraniumshells"
    })
}

/// Apply Uranium residual damage mult when upgrade present.
pub fn overlord_damage_with_uranium(base_damage: f32, has_uranium: bool) -> f32 {
    if has_uranium {
        base_damage * OVERLORD_URANIUM_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (primary_damage, secondary_damage) residual rings with optional Uranium.
pub fn overlord_ring_damage(has_uranium: bool) -> (f32, f32) {
    (
        overlord_damage_with_uranium(OVERLORD_PRIMARY_DAMAGE, has_uranium),
        overlord_damage_with_uranium(OVERLORD_SECONDARY_DAMAGE, has_uranium),
    )
}

/// Build residual PRIMARY OverlordTankGun Weapon.
pub fn overlord_gun_weapon(has_uranium: bool) -> Weapon {
    let (primary, _sec) = overlord_ring_damage(has_uranium);
    Weapon {
        damage: primary,
        range: OVERLORD_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(OVERLORD_RELOAD_FRAMES),
        last_fire_time: 0.0,
        // ClipSize honesty residual (not full dual-shot cadence).
        ammo: Some(OVERLORD_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: OVERLORD_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
    }
}

/// Dual-radius residual damage at distance from impact (max of rings).
///
/// Intended target at impact takes PrimaryDamage; nearby units within
/// PrimaryDamageRadius take PrimaryDamage; SecondaryDamageRadius takes
/// SecondaryDamage residual.
pub fn overlord_damage_at(distance_from_impact: f32, has_uranium: bool) -> f32 {
    let (primary, secondary) = overlord_ring_damage(has_uranium);
    if distance_from_impact <= OVERLORD_PRIMARY_RADIUS {
        primary
    } else if distance_from_impact <= OVERLORD_SECONDARY_RADIUS {
        secondary
    } else {
        0.0
    }
}

/// Legal residual splash target.
///
/// Retail RadiusDamageAffects = ALLIES ENEMIES NEUTRALS (friendly-fire residual).
/// Host residual still skips self-source and under-construction.
pub fn is_legal_overlord_gun_splash_target(
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
    fn overlord_gun_name_matrix() {
        assert!(is_overlord_gun_chassis("ChinaTankOverlord"));
        assert!(is_overlord_gun_chassis("China_OverlordTank"));
        assert!(is_overlord_gun_chassis("TestOverlord"));
        assert!(is_overlord_gun_chassis("Nuke_ChinaTankOverlord"));
        assert!(is_overlord_gun_chassis("Tank_ChinaTankEmperor"));
        assert!(is_overlord_gun_chassis("TestEmperor"));
        assert!(!is_overlord_gun_chassis("ChinaTankOverlordGattlingCannon"));
        assert!(!is_overlord_gun_chassis("ChinaTankOverlordPropagandaTower"));
        assert!(!is_overlord_gun_chassis("ChinaTankOverlordBattleBunker"));
        assert!(!is_overlord_gun_chassis("ChinaVehicleHelix"));
        assert!(!is_overlord_gun_chassis("OverlordTankShell"));
        assert!(!is_overlord_gun_chassis("OverlordTankGun"));
        assert!(!is_overlord_gun_chassis("ChinaTankBattleMaster"));
    }

    #[test]
    fn weapon_uranium_and_dual_radius() {
        let w = overlord_gun_weapon(false);
        assert!((w.damage - 80.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.05);
        assert_eq!(w.ammo, Some(2));

        let wu = overlord_gun_weapon(true);
        assert!((wu.damage - 100.0).abs() < 0.01);

        assert!((overlord_damage_at(0.0, false) - 80.0).abs() < 0.01);
        assert!((overlord_damage_at(5.0, false) - 80.0).abs() < 0.01);
        assert!((overlord_damage_at(8.0, false) - 20.0).abs() < 0.01);
        assert!((overlord_damage_at(12.0, false)).abs() < 0.01);
        assert!((overlord_damage_at(8.0, true) - 25.0).abs() < 0.01);

        assert!(should_apply_overlord_gun_residual(true, false));
        assert!(!should_apply_overlord_gun_residual(true, true));
        assert!(!should_apply_overlord_gun_residual(false, false));
    }
}
