//! Host GLA Marauder Tank residual (salvage fire-rate tiers).
//!
//! Residual slice (playability):
//! - Spawns with PRIMARY `MarauderTankGun` (dmg **60** / radius **5** / range **170**
//!   / DelayBetweenShots **2000**ms).
//! - Salvage weapon upgrade residual (`WEAPON_SALVAGER` / CRATEUPGRADE):
//!   - Tier 0: `MarauderTankGun` — 2000ms delay (60 frames @ 30 FPS)
//!   - Tier 1 CRATEUPGRADE_ONE: `MarauderTankGunUpgradeOne` — 1500ms (45 frames),
//!     WeaponSpeed residual 400
//!   - Tier 2 CRATEUPGRADE_TWO: `MarauderTankGunUpgradeTwo` — 750ms (23 frames),
//!     ClipSize 2 residual, WeaponSpeed residual 500
//! - PrimaryDamage stays **60** across tiers (fire-rate residual, not damage).
//! - Small PrimaryDamageRadius **5** splash residual on fire.
//!
//! Fail-closed honesty:
//! - Not full SalvageCrate collate / W3D turret subobject (Turret / TurretUp01/02) swap
//! - Not full ClipReloadTime 100ms dual-shot cadence matrix (tier 2 uses faster reload)
//! - Not Min/MaxTargetPitch / ScatterRadiusVsInfantry matrix
//! - Not network salvage / weapon-set replication (network deferred)

use super::Weapon;

/// Retail primary base weapon.
pub const MARAUDER_TANK_GUN: &str = "MarauderTankGun";
/// Retail CRATEUPGRADE_ONE weapon (faster fire).
pub const MARAUDER_TANK_GUN_UPGRADE_ONE: &str = "MarauderTankGunUpgradeOne";
/// Retail CRATEUPGRADE_TWO weapon (fastest dual residual).
pub const MARAUDER_TANK_GUN_UPGRADE_TWO: &str = "MarauderTankGunUpgradeTwo";

/// Retail PrimaryDamage (all tiers).
pub const MARAUDER_DAMAGE: f32 = 60.0;
/// Retail PrimaryDamageRadius (all tiers).
pub const MARAUDER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange (all tiers).
pub const MARAUDER_RANGE: f32 = 170.0;

/// Tier 0 DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const MARAUDER_DELAY_FRAMES_TIER0: u32 = 60;
/// Tier 1 DelayBetweenShots 1500ms → 45 frames @ 30 FPS.
pub const MARAUDER_DELAY_FRAMES_TIER1: u32 = 45;
/// Tier 2 DelayBetweenShots 750ms → 23 frames @ 30 FPS (ceil 22.5).
pub const MARAUDER_DELAY_FRAMES_TIER2: u32 = 23;

/// WeaponSpeed residual (shell flight residual; host hits are still residual-instant).
pub const MARAUDER_SPEED_TIER0: f32 = 300.0;
pub const MARAUDER_SPEED_TIER1: f32 = 400.0;
pub const MARAUDER_SPEED_TIER2: f32 = 500.0;

/// Residual fire audio.
pub const MARAUDER_FIRE_AUDIO: &str = "MarauderTankWeapon";

/// Multi-weapon salvage residual tier (WEAPONSET_CRATEUPGRADE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarauderWeaponTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl MarauderWeaponTier {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::One,
            2 => Self::Two,
            _ => Self::Base,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

/// Whether template is a residual Marauder tank vehicle.
///
/// Fail-closed: name residual (not full Salvage / W3D turret matrix).
/// Excludes weapons / projectiles / science tokens.
pub fn is_marauder_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Weapon / shell / upgrade / science tokens are not the living vehicle residual.
    if n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("gun")
    {
        return false;
    }
    n.contains("marauder")
        || n == "gla_maraudertank"
        || n == "testmarauder"
        || n.contains("tankmarauder")
}

/// Weapon template name for salvage tier.
pub fn marauder_weapon_name_for_tier(tier: MarauderWeaponTier) -> &'static str {
    match tier {
        MarauderWeaponTier::Base => MARAUDER_TANK_GUN,
        MarauderWeaponTier::One => MARAUDER_TANK_GUN_UPGRADE_ONE,
        MarauderWeaponTier::Two => MARAUDER_TANK_GUN_UPGRADE_TWO,
    }
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for salvage tier.
pub fn marauder_weapon_stats(tier: MarauderWeaponTier) -> (f32, f32, u32, f32, f32) {
    match tier {
        MarauderWeaponTier::Base => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER0,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER0,
        ),
        MarauderWeaponTier::One => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER1,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER1,
        ),
        MarauderWeaponTier::Two => (
            MARAUDER_DAMAGE,
            MARAUDER_RANGE,
            MARAUDER_DELAY_FRAMES_TIER2,
            MARAUDER_SPLASH_RADIUS,
            MARAUDER_SPEED_TIER2,
        ),
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Weapon for a salvage tier.
pub fn marauder_weapon_for_tier(tier: MarauderWeaponTier) -> Weapon {
    let (damage, range, delay, _splash, speed) = marauder_weapon_stats(tier);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: speed,
        pre_attack_delay: 0.0,
    }
}

/// Splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn marauder_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
) -> f32 {
    if is_intended_target {
        return MARAUDER_DAMAGE;
    }
    if distance_from_impact <= MARAUDER_SPLASH_RADIUS {
        MARAUDER_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_marauder_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Marauder residual path.
pub fn should_apply_marauder_residual(is_marauder: bool) -> bool {
    is_marauder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marauder_name_matrix() {
        assert!(is_marauder_template("GLATankMarauder"));
        assert!(is_marauder_template("GLA_MarauderTank"));
        assert!(is_marauder_template("TestMarauder"));
        assert!(is_marauder_template("Chem_GLATankMarauder"));
        assert!(is_marauder_template("Demo_GLATankMarauder"));
        assert!(is_marauder_template("Slth_GLATankMarauder"));
        assert!(!is_marauder_template("MarauderTankGun"));
        assert!(!is_marauder_template("MarauderTankGunUpgradeOne"));
        assert!(!is_marauder_template("MarauderTankShell"));
        assert!(!is_marauder_template("GLAVehicleRocketBuggy"));
        assert!(!is_marauder_template("USA_Ranger"));
        assert!(!is_marauder_template("SCIENCE_MarauderTank"));
    }

    #[test]
    fn fire_rate_tiers_same_damage_faster_reload() {
        let (d0, r0, f0, s0, sp0) = marauder_weapon_stats(MarauderWeaponTier::Base);
        let (d1, r1, f1, s1, sp1) = marauder_weapon_stats(MarauderWeaponTier::One);
        let (d2, r2, f2, s2, sp2) = marauder_weapon_stats(MarauderWeaponTier::Two);

        assert!((d0 - 60.0).abs() < 0.01);
        assert!((d1 - 60.0).abs() < 0.01);
        assert!((d2 - 60.0).abs() < 0.01);
        assert!((r0 - 170.0).abs() < 0.01);
        assert!((r1 - r0).abs() < 0.01);
        assert!((r2 - r0).abs() < 0.01);
        assert!((s0 - 5.0).abs() < 0.01);
        assert!((s1 - s0).abs() < 0.01);
        assert!((s2 - s0).abs() < 0.01);

        // Fire-rate residual: each tier is strictly faster.
        assert!(f1 < f0);
        assert!(f2 < f1);
        assert_eq!(f0, 60);
        assert_eq!(f1, 45);
        assert_eq!(f2, 23);

        assert!((sp0 - 300.0).abs() < 0.01);
        assert!((sp1 - 400.0).abs() < 0.01);
        assert!((sp2 - 500.0).abs() < 0.01);

        assert_eq!(
            marauder_weapon_name_for_tier(MarauderWeaponTier::Two),
            MARAUDER_TANK_GUN_UPGRADE_TWO
        );

        let w2 = marauder_weapon_for_tier(MarauderWeaponTier::Two);
        assert!((w2.damage - 60.0).abs() < 0.01);
        assert!(w2.reload_time < marauder_weapon_for_tier(MarauderWeaponTier::Base).reload_time);
    }

    #[test]
    fn splash_residual() {
        assert!((marauder_splash_damage_at(true, 0.0) - 60.0).abs() < 0.01);
        assert!((marauder_splash_damage_at(false, 4.0) - 60.0).abs() < 0.01);
        assert!((marauder_splash_damage_at(false, 6.0)).abs() < 0.01);
        assert!(should_apply_marauder_residual(true));
        assert!(!should_apply_marauder_residual(false));
    }
}
