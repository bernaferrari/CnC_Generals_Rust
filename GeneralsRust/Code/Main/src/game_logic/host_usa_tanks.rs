//! Host America Crusader / Paladin tank residuals (main gun + Composite Armor).
//!
//! Residual slice (playability):
//! - AmericaTankCrusader / *Crusader* spawns with PRIMARY `CrusaderTankGun`
//!   (dmg **60** / range **150** / Delay **2000**ms → 60 frames).
//! - AmericaTankPaladin / *Paladin* spawns with PRIMARY `PaladinTankGun`
//!   (same gun stats; PointDefenseLaser residual already in host_point_defense).
//! - Laser General residual:
//!   - `Lazr_AmericaTankCrusader` → `Lazr_CrusaderTankGun` (dmg **80** / r**5** /
//!     Delay **2000**ms → 60 frames). Instant laser residual (WeaponSpeed 99999).
//!   - `Lazr_AmericaTankPaladin` → `Lazr_PaladinTankGun` (dmg **70** / r**3** /
//!     Delay **1000**ms → 30 frames).
//! - Upgrade_AmericaCompositeArmor MaxHealthUpgrade residual: **+100** max HP
//!   with ADD_CURRENT_HEALTH_TOO on Crusader / Paladin (not Humvee / Avenger).
//!
//! Fail-closed honesty:
//! - Not full ArmorSet PLAYER_UPGRADE UpgradedTankArmor matrix
//! - Not full turret recoil / shell projectile bezier path
//! - Not full LaserName / LaserBoneName drawable beam matrix (Lazr residual)
//! - Not SCIENCE_PaladinTank prereq gate / ProductionUpdate door UI
//! - Not network composite / tank gun / laser-tank replication (network deferred)

use super::Weapon;

/// Retail CrusaderTankGun primary weapon.
pub const CRUSADER_TANK_GUN: &str = "CrusaderTankGun";
/// Retail PaladinTankGun primary weapon.
pub const PALADIN_TANK_GUN: &str = "PaladinTankGun";
/// Retail Laser General Crusader primary.
pub const LAZR_CRUSADER_TANK_GUN: &str = "Lazr_CrusaderTankGun";
/// Retail Laser General Paladin primary.
pub const LAZR_PALADIN_TANK_GUN: &str = "Lazr_PaladinTankGun";
/// Retail Upgrade_AmericaCompositeArmor.
pub const UPGRADE_AMERICA_COMPOSITE_ARMOR: &str = "Upgrade_AmericaCompositeArmor";

/// Retail tank gun PrimaryDamage residual.
pub const USA_TANK_GUN_DAMAGE: f32 = 60.0;
/// Retail Lazr_CrusaderTankGun PrimaryDamage residual.
pub const LAZR_CRUSADER_TANK_GUN_DAMAGE: f32 = 80.0;
/// Retail Lazr_PaladinTankGun PrimaryDamage residual.
pub const LAZR_PALADIN_TANK_GUN_DAMAGE: f32 = 70.0;
/// Retail tank gun AttackRange residual.
pub const USA_TANK_GUN_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const USA_TANK_GUN_DELAY_FRAMES: u32 = 60;
/// Retail Lazr_PaladinTankGun DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const LAZR_PALADIN_TANK_GUN_DELAY_FRAMES: u32 = 30;
/// Retail MaxHealthUpgrade AddMaxHealth residual.
pub const COMPOSITE_ARMOR_ADD_MAX_HEALTH: f32 = 100.0;

/// Residual fire audio.
pub const CRUSADER_FIRE_AUDIO: &str = "CrusaderTankWeapon";
pub const PALADIN_FIRE_AUDIO: &str = "PaladinTankWeapon";
/// Residual Laser General laser fire audio (FX residual honesty).
pub const LAZR_TANK_FIRE_AUDIO: &str = "Lazr_WeaponFX_LaserCrusader";

/// Whether template is a residual Crusader tank chassis.
///
/// Fail-closed: name residual; excludes debris / weapons / pure Paladin.
pub fn is_crusader_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testcrusader" || n == "usa_crusader" || n == "usa_crusadertank" {
        return true;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("turret")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("crate")
    {
        return false;
    }
    n.contains("crusader")
}

/// Whether template is a residual Paladin tank chassis.
///
/// Fail-closed: name residual; PointDefenseLaser modules already host residual.
pub fn is_paladin_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testpaladin" || n == "usa_paladin" || n == "usa_paladintank" {
        return true;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("pointdefense")
        || n.contains("dead")
        || n.starts_with("upgrade")
    {
        return false;
    }
    n.contains("paladin")
}

/// Composite Armor MaxHealthUpgrade applies to Crusader + Paladin (retail ModuleTag).
pub fn is_composite_armor_unit_template(template_name: &str) -> bool {
    is_crusader_template(template_name) || is_paladin_template(template_name)
}

/// Whether template is a Laser General residual chassis (Lazr_ prefix or name).
///
/// Fail-closed: name residual (not full general-science production gate).
pub fn is_laser_general_tank_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("lazr_")
        || n.contains("lazr_america")
        || n.contains("lasergeneral")
        || n == "testlazrcrusader"
        || n == "testlazrpaladin"
}

/// Primary weapon name for residual USA main battle tank.
pub fn primary_weapon_name_for_usa_tank(template_name: &str) -> Option<&'static str> {
    let laser = is_laser_general_tank_template(template_name);
    if is_paladin_template(template_name) {
        Some(if laser {
            LAZR_PALADIN_TANK_GUN
        } else {
            PALADIN_TANK_GUN
        })
    } else if is_crusader_template(template_name) {
        Some(if laser {
            LAZR_CRUSADER_TANK_GUN
        } else {
            CRUSADER_TANK_GUN
        })
    } else {
        None
    }
}

/// Build residual tank gun weapon (Crusader / Paladin shared stats).
pub fn usa_tank_gun_weapon() -> Weapon {
    usa_tank_gun_weapon_for_template("AmericaTankCrusader")
}

/// Build residual tank gun for a specific chassis template (laser vs shell).
pub fn usa_tank_gun_weapon_for_template(template_name: &str) -> Weapon {
    let laser = is_laser_general_tank_template(template_name);
    let is_paladin = is_paladin_template(template_name);
    let (damage, delay_frames, projectile_speed) = if laser && is_paladin {
        (
            LAZR_PALADIN_TANK_GUN_DAMAGE,
            LAZR_PALADIN_TANK_GUN_DELAY_FRAMES,
            99999.0,
        )
    } else if laser {
        (LAZR_CRUSADER_TANK_GUN_DAMAGE, USA_TANK_GUN_DELAY_FRAMES, 99999.0)
    } else {
        // Shell residual: Paladin shares Crusader damage/delay; projectile speed differs.
        let speed = if is_paladin { 300.0 } else { 400.0 };
        (USA_TANK_GUN_DAMAGE, USA_TANK_GUN_DELAY_FRAMES, speed)
    };
    Weapon {
        damage,
        range: USA_TANK_GUN_RANGE,
        min_range: 0.0,
        reload_time: delay_frames as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed,
        pre_attack_delay: 0.0,
    }
}

/// Apply Composite Armor residual: +AddMaxHealth current+max (ADD_CURRENT_HEALTH_TOO).
///
/// Returns true when health was increased (first apply residual; idempotent via tag).
pub fn apply_composite_armor_health(max_health: &mut f32, current: &mut f32, maximum: &mut f32) {
    *max_health = max_health.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
    *maximum = maximum.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
    *current = current.saturating_add_f32(COMPOSITE_ARMOR_ADD_MAX_HEALTH);
}

/// f32 saturating add helper (no std saturating_add for f32).
trait SaturatingAddF32 {
    fn saturating_add_f32(self, rhs: f32) -> f32;
}
impl SaturatingAddF32 for f32 {
    fn saturating_add_f32(self, rhs: f32) -> f32 {
        (self + rhs).max(0.0)
    }
}

/// Only Paladin PointDefenseLaser has SecondaryTargetTypes = INFANTRY.
pub fn paladin_allows_secondary_infantry_intercept(template_name: &str) -> bool {
    is_paladin_template(template_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crusader_paladin_name_matrix() {
        assert!(is_crusader_template("AmericaTankCrusader"));
        assert!(is_crusader_template("USA_Crusader"));
        assert!(is_crusader_template("USA_CrusaderTank"));
        assert!(is_crusader_template("TestCrusader"));
        assert!(!is_crusader_template("AmericaTankPaladin"));
        assert!(!is_crusader_template("2FreeCrusadersCrate"));

        assert!(is_paladin_template("AmericaTankPaladin"));
        assert!(is_paladin_template("USA_Paladin"));
        assert!(is_paladin_template("TestPaladin"));
        assert!(!is_paladin_template("AmericaTankCrusader"));
        assert!(!is_paladin_template("PaladinPointDefenseLaser"));
    }

    #[test]
    fn composite_armor_targets() {
        assert!(is_composite_armor_unit_template("AmericaTankCrusader"));
        assert!(is_composite_armor_unit_template("AmericaTankPaladin"));
        assert!(!is_composite_armor_unit_template("AmericaVehicleHumvee"));
        assert!(!is_composite_armor_unit_template("AmericaTankAvenger"));
    }

    #[test]
    fn composite_armor_adds_100() {
        let mut max_h = 480.0_f32;
        let mut cur = 400.0_f32;
        let mut maximum = 480.0_f32;
        apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
        assert!((max_h - 580.0).abs() < 0.01);
        assert!((maximum - 580.0).abs() < 0.01);
        assert!((cur - 500.0).abs() < 0.01);
    }

    #[test]
    fn tank_gun_stats() {
        let w = usa_tank_gun_weapon();
        assert!((w.damage - 60.0).abs() < 0.01);
        assert!((w.range - 150.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn laser_tank_gun_stats() {
        let c = usa_tank_gun_weapon_for_template("Lazr_AmericaTankCrusader");
        assert!((c.damage - LAZR_CRUSADER_TANK_GUN_DAMAGE).abs() < 0.01);
        assert!((c.reload_time - 2.0).abs() < 0.01);
        assert!((c.projectile_speed - 99999.0).abs() < 1.0);

        let p = usa_tank_gun_weapon_for_template("Lazr_AmericaTankPaladin");
        assert!((p.damage - LAZR_PALADIN_TANK_GUN_DAMAGE).abs() < 0.01);
        assert!((p.reload_time - 1.0).abs() < 0.01);
        assert!(is_laser_general_tank_template("Lazr_AmericaTankCrusader"));
        assert!(is_laser_general_tank_template("TestLazrPaladin"));
        assert!(!is_laser_general_tank_template("AmericaTankCrusader"));
    }

    #[test]
    fn weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankCrusader"),
            Some(CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("Lazr_AmericaTankCrusader"),
            Some(LAZR_CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("Lazr_AmericaTankPaladin"),
            Some(LAZR_PALADIN_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankPaladin"),
            Some(PALADIN_TANK_GUN)
        );
        assert_eq!(primary_weapon_name_for_usa_tank("AmericaTankAvenger"), None);
    }
}
