//! Host America Crusader / Paladin tank residuals (main gun + Composite Armor).
//!
//! Residual slice (playability):
//! - AmericaTankCrusader / *Crusader* spawns with PRIMARY `CrusaderTankGun`
//!   (dmg **60** / range **150** / Delay **2000**ms → 60 frames).
//! - AmericaTankPaladin / *Paladin* spawns with PRIMARY `PaladinTankGun`
//!   (same gun stats; PointDefenseLaser residual already in host_point_defense).
//! - Upgrade_AmericaCompositeArmor MaxHealthUpgrade residual: **+100** max HP
//!   with ADD_CURRENT_HEALTH_TOO on Crusader / Paladin (not Humvee / Avenger).
//!
//! Fail-closed honesty:
//! - Not full ArmorSet PLAYER_UPGRADE UpgradedTankArmor matrix
//! - Not full turret recoil / shell projectile bezier path
//! - Not SCIENCE_PaladinTank prereq gate / ProductionUpdate door UI
//! - Not network composite / tank gun replication (network deferred)

use super::Weapon;

/// Retail CrusaderTankGun primary weapon.
pub const CRUSADER_TANK_GUN: &str = "CrusaderTankGun";
/// Retail PaladinTankGun primary weapon.
pub const PALADIN_TANK_GUN: &str = "PaladinTankGun";
/// Retail Upgrade_AmericaCompositeArmor.
pub const UPGRADE_AMERICA_COMPOSITE_ARMOR: &str = "Upgrade_AmericaCompositeArmor";

/// Retail tank gun PrimaryDamage residual.
pub const USA_TANK_GUN_DAMAGE: f32 = 60.0;
/// Retail tank gun AttackRange residual.
pub const USA_TANK_GUN_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const USA_TANK_GUN_DELAY_FRAMES: u32 = 60;
/// Retail MaxHealthUpgrade AddMaxHealth residual.
pub const COMPOSITE_ARMOR_ADD_MAX_HEALTH: f32 = 100.0;

/// Residual fire audio.
pub const CRUSADER_FIRE_AUDIO: &str = "CrusaderTankWeapon";
pub const PALADIN_FIRE_AUDIO: &str = "PaladinTankWeapon";

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

/// Primary weapon name for residual USA main battle tank.
pub fn primary_weapon_name_for_usa_tank(template_name: &str) -> Option<&'static str> {
    if is_paladin_template(template_name) {
        Some(PALADIN_TANK_GUN)
    } else if is_crusader_template(template_name) {
        Some(CRUSADER_TANK_GUN)
    } else {
        None
    }
}

/// Build residual tank gun weapon (Crusader / Paladin shared stats).
pub fn usa_tank_gun_weapon() -> Weapon {
    Weapon {
        damage: USA_TANK_GUN_DAMAGE,
        range: USA_TANK_GUN_RANGE,
        min_range: 0.0,
        reload_time: USA_TANK_GUN_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 400.0,
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
    fn weapon_name_lookup() {
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankCrusader"),
            Some(CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_usa_tank("AmericaTankPaladin"),
            Some(PALADIN_TANK_GUN)
        );
        assert_eq!(primary_weapon_name_for_usa_tank("AmericaTankAvenger"), None);
    }
}
