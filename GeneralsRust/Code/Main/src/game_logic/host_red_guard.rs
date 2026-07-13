//! Host China Red Guard residual (machine gun + horde/nationalism ROF + bayonet).
//!
//! Residual slice (playability):
//! - `ChinaInfantryRedguard` / variants spawn with PRIMARY `RedguardMachineGun`
//!   (dmg **15** / range **100** / Delay **1000**ms → 30 frames).
//! - Horde residual (`HordeUpdate` KindOf INFANTRY, AlliesOnly, ExactMatch=No,
//!   Radius **30**, Count **5**): WeaponBonus HORDE RATE_OF_FIRE **150%**
//!   → delay floor(30/1.5)=**20** frames.
//! - Nationalism residual (`Upgrade_Nationalism` while in horde):
//!   additional RATE_OF_FIRE **125%** (stacks) → floor(30/1.875)=**16** frames.
//! - Bayonet residual (`RedguardBayonet` stats): when attacking infantry within
//!   AttackRange **2**, one-shot MELEE residual (PrimaryDamage **10000**).
//!   Retail ZH WeaponSet is PRIMARY-only; bayonet is residual from weapon def +
//!   PREATTACK_C/FIRING_C animations (CINE units bind TERTIARY).
//!
//! Fail-closed honesty:
//! - Not full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
//! - Not full Fanaticism infantry-general nationalism branch
//! - Not full WeaponSet tertiary auto-choose / pre-attack anim lock matrix
//! - Not SCIENCE_RedGuardTraining elite spawn residual
//! - Not network horde / nationalism replication (network deferred)

use super::Weapon;
use crate::game_logic::host_battlemaster::{
    has_nationalism_upgrade, UPGRADE_NATIONALISM,
};

// Re-export nationalism helpers for integration call sites.
pub use crate::game_logic::host_battlemaster::{has_nationalism_upgrade as red_guard_has_nationalism};
pub use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM as RED_GUARD_UPGRADE_NATIONALISM;

/// Retail primary weapon.
pub const REDGUARD_MACHINE_GUN: &str = "RedguardMachineGun";
/// Residual bayonet weapon name.
pub const REDGUARD_BAYONET: &str = "RedguardBayonet";

/// Retail PrimaryDamage base.
pub const REDGUARD_DAMAGE: f32 = 15.0;
/// Retail AttackRange.
pub const REDGUARD_RANGE: f32 = 100.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const REDGUARD_BASE_DELAY_FRAMES: u32 = 30;

/// Bayonet PrimaryDamage residual (one-shot kill).
pub const BAYONET_DAMAGE: f32 = 10_000.0;
/// Bayonet AttackRange residual.
pub const BAYONET_RANGE: f32 = 2.0;
/// Bayonet DelayBetweenShots 1900ms → 57 frames @ 30 FPS.
pub const BAYONET_DELAY_FRAMES: u32 = 57;
/// Bayonet PreAttackDelay 1400ms residual (fail-closed vs full pre-attack lock).
pub const BAYONET_PRE_ATTACK_FRAMES: u32 = 42;

/// HORDE WeaponBonus RATE_OF_FIRE 150%.
pub const INFANTRY_HORDE_ROF_MULT: f32 = 1.5;
/// NATIONALISM WeaponBonus RATE_OF_FIRE 125% (stacks with horde when both active).
pub const INFANTRY_NATIONALISM_ROF_MULT: f32 = 1.25;

/// Retail HordeUpdate Radius for China infantry (Red Guard / Tank Hunter).
pub const INFANTRY_HORDE_RADIUS: f32 = 30.0;
/// Retail HordeUpdate Count (includes self via C++ minCount-1 others).
pub const INFANTRY_HORDE_COUNT: u32 = 5;
/// Retail HordeUpdate UpdateRate 1000ms → 30 frames @ 30 FPS.
pub const INFANTRY_HORDE_UPDATE_FRAMES: u32 = 30;

/// Residual fire audio.
pub const REDGUARD_FIRE_AUDIO: &str = "RedGuardWeapon";
/// Residual bayonet audio.
pub const BAYONET_FIRE_AUDIO: &str = "HeroUSAKnifeAttack";

/// Whether template is a residual Red Guard infantry.
///
/// Fail-closed: name residual. Excludes weapons/science/debris tokens.
pub fn is_red_guard_template(template_name: &str) -> bool {
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
        || n.contains("training")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("bayonet")
        || n.contains("machinegun")
        || n.contains("machine_gun")
    {
        return false;
    }
    n.contains("redguard")
        || n.contains("red_guard")
        || n == "china_soldier"
        || n == "testredguard"
}

/// Whether residual unit participates in China infantry HordeUpdate residual
/// (Red Guard + Tank Hunter share KindOf INFANTRY horde params).
///
/// Name residual only (avoids circular host_tank_hunter import).
pub fn is_china_infantry_horde_unit(template_name: &str) -> bool {
    if is_red_guard_template(template_name) {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("locomotor")
        || n.contains("sticky")
        || n.contains("detonation")
    {
        return false;
    }
    n.contains("tankhunter") || n.contains("tank_hunter") || n == "testtankhunter"
}

/// Combined ROF multiplier residual (HORDE * NATIONALISM when both active).
///
/// Nationalism only applies while in horde (C++ AIUpdate evaluateMoraleBonus).
pub fn red_guard_rof_multiplier(in_horde: bool, has_nationalism: bool) -> f32 {
    let mut rof = 1.0_f32;
    if in_horde {
        rof *= INFANTRY_HORDE_ROF_MULT;
        if has_nationalism {
            rof *= INFANTRY_NATIONALISM_ROF_MULT;
        }
    }
    rof
}

/// Delay frames residual: floor(base / ROF), min 1.
pub fn red_guard_delay_frames(in_horde: bool, has_nationalism: bool) -> u32 {
    let base = REDGUARD_BASE_DELAY_FRAMES as f32;
    let rof = red_guard_rof_multiplier(in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (damage, range, delay_frames) for gun residual with ROF bonuses.
pub fn red_guard_weapon_stats(in_horde: bool, has_nationalism: bool) -> (f32, f32, u32) {
    (
        REDGUARD_DAMAGE,
        REDGUARD_RANGE,
        red_guard_delay_frames(in_horde, has_nationalism),
    )
}

/// Build residual PRIMARY machine-gun Weapon with horde/nationalism ROF residual.
pub fn red_guard_weapon(in_horde: bool, has_nationalism: bool) -> Weapon {
    let (damage, range, delay) = red_guard_weapon_stats(in_horde, has_nationalism);
    Weapon {
        damage,
        range,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Residual bayonet Weapon (close-range one-shot).
pub fn red_guard_bayonet_weapon() -> Weapon {
    Weapon {
        damage: BAYONET_DAMAGE,
        range: BAYONET_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(BAYONET_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: delay_frames_to_reload_secs(BAYONET_PRE_ATTACK_FRAMES),
    }
}

/// Whether bayonet residual should apply for this shot.
///
/// Residual: target is living infantry, horizontal distance ≤ BAYONET_RANGE.
pub fn should_apply_bayonet_residual(
    is_red_guard: bool,
    target_is_infantry: bool,
    target_alive: bool,
    distance: f32,
) -> bool {
    is_red_guard && target_is_infantry && target_alive && distance <= BAYONET_RANGE && distance >= 0.0
}

/// Horde residual: Count includes self (C++: others >= Count-1).
pub fn is_in_infantry_horde(nearby_infantry_allies: u32) -> bool {
    nearby_infantry_allies + 1 >= INFANTRY_HORDE_COUNT
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether other unit counts toward self's infantry horde residual
/// (KindOf INFANTRY + AlliesOnly + Radius; ExactMatch=No).
pub fn counts_toward_infantry_horde(
    self_alive: bool,
    other_alive: bool,
    same_team: bool,
    other_is_infantry: bool,
    distance: f32,
    radius: f32,
) -> bool {
    self_alive
        && other_alive
        && same_team
        && other_is_infantry
        && distance <= radius
        && distance >= 0.0
}

/// Whether residual fire should apply Red Guard residual path (gun honesty).
pub fn should_apply_red_guard_residual(is_red_guard: bool) -> bool {
    is_red_guard
}

/// Re-export nationalism tag helper for call sites.
pub fn apply_nationalism_tag_detect(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    has_nationalism_upgrade(applied_upgrades)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn red_guard_name_matrix() {
        assert!(is_red_guard_template("ChinaInfantryRedguard"));
        assert!(is_red_guard_template("China_RedGuard"));
        assert!(is_red_guard_template("Tank_ChinaInfantryRedguard"));
        assert!(is_red_guard_template("Nuke_ChinaInfantryRedguard"));
        assert!(is_red_guard_template("TestRedGuard"));
        assert!(is_red_guard_template("China_Soldier"));
        assert!(!is_red_guard_template("RedguardMachineGun"));
        assert!(!is_red_guard_template("RedguardBayonet"));
        assert!(!is_red_guard_template("SCIENCE_RedGuardTraining"));
        assert!(!is_red_guard_template("ChinaInfantryTankHunter"));
        assert!(!is_red_guard_template("RedguardLocomotor"));
        assert!(!is_red_guard_template("Upgrade_Nationalism"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f) = red_guard_weapon_stats(false, false);
        assert!((d - 15.0).abs() < 0.01);
        assert!((r - 100.0).abs() < 0.01);
        assert_eq!(f, 30);
        let w = red_guard_weapon(false, false);
        assert!((w.damage - 15.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn horde_and_nationalism_rof_stack() {
        // HORDE alone: floor(30/1.5)=20
        assert_eq!(red_guard_delay_frames(true, false), 20);
        // Nationalism alone without horde does nothing residual.
        assert_eq!(red_guard_delay_frames(false, true), 30);
        // HORDE + NATIONALISM: floor(30/1.875)=16
        assert_eq!(red_guard_delay_frames(true, true), 16);

        let w_horde = red_guard_weapon(true, false);
        let w_both = red_guard_weapon(true, true);
        let w_base = red_guard_weapon(false, false);
        assert!(w_horde.reload_time < w_base.reload_time - 0.05);
        assert!(w_both.reload_time < w_horde.reload_time - 0.01);
    }

    #[test]
    fn horde_count_includes_self() {
        assert!(is_in_infantry_horde(4));
        assert!(!is_in_infantry_horde(3));
        assert!(!is_in_infantry_horde(0));
        assert!(is_in_infantry_horde(5));
    }

    #[test]
    fn bayonet_residual_filters() {
        assert!(should_apply_bayonet_residual(true, true, true, 1.5));
        assert!(should_apply_bayonet_residual(true, true, true, 2.0));
        assert!(!should_apply_bayonet_residual(true, true, true, 2.1));
        assert!(!should_apply_bayonet_residual(true, false, true, 1.0));
        assert!(!should_apply_bayonet_residual(false, true, true, 1.0));
        assert!(!should_apply_bayonet_residual(true, true, false, 1.0));
        let w = red_guard_bayonet_weapon();
        assert!((w.damage - 10_000.0).abs() < 0.1);
        assert!((w.range - 2.0).abs() < 0.01);
    }

    #[test]
    fn nationalism_tag_detect() {
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_NATIONALISM.to_string());
        assert!(has_nationalism_upgrade(&tags));
        assert!(apply_nationalism_tag_detect(&tags));
    }

    #[test]
    fn counts_toward_infantry_horde_filters() {
        assert!(counts_toward_infantry_horde(
            true, true, true, true, 20.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, false, true, 20.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, true, true, 40.0, 30.0
        ));
        assert!(!counts_toward_infantry_horde(
            true, true, true, false, 10.0, 30.0
        ));
    }
}
