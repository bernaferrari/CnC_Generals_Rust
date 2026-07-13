//! Host China Battlemaster tank residual (main gun + Uranium Shells + horde/nationalism ROF).
//!
//! Residual slice (playability):
//! - `ChinaTankBattleMaster` / variants spawn with PRIMARY `BattleMasterTankGun`
//!   (dmg **60** / radius **5** / range **150** / Delay **2000**ms → 60 frames).
//! - Uranium Shells PLAYER_UPGRADE residual (`Upgrade_ChinaUraniumShells`):
//!   WeaponBonus DAMAGE **125%** → PrimaryDamage **75**.
//! - Horde residual (`HordeUpdate` ExactMatch VEHICLE allies, Radius **75**, Count **5**):
//!   WeaponBonus HORDE RATE_OF_FIRE **150%** → delay floor(60/1.5)=**40** frames.
//! - Nationalism residual (`Upgrade_Nationalism` while in horde):
//!   additional RATE_OF_FIRE **125%** (stacks with horde) → delay floor(60/(1.5*1.25))=**32** frames.
//! - Small PrimaryDamageRadius **5** splash residual on fire.
//!
//! Fail-closed honesty:
//! - Not full HordeUpdate RubOffRadius honorary-member / terrain-decal flag matrix
//! - Not full Fanaticism infantry-general nationalism branch
//! - Not full Nuclear Tanks death weapon / locomotor upgrade residual
//! - SCIENCE_BattlemasterTraining ELITE spawn residual closed in host_unit_training
//! - Not network uranium / horde replication (network deferred)

use super::Weapon;

/// Retail primary weapon.
pub const BATTLE_MASTER_TANK_GUN: &str = "BattleMasterTankGun";
/// Retail Upgrade_ChinaUraniumShells (WeaponBonusUpgrade → PLAYER_UPGRADE).
pub const UPGRADE_CHINA_URANIUM_SHELLS: &str = "Upgrade_ChinaUraniumShells";
/// Retail Upgrade_Nationalism (player science/upgrade; stacks with HORDE).
pub const UPGRADE_NATIONALISM: &str = "Upgrade_Nationalism";

/// Retail PrimaryDamage base.
pub const BATTLE_MASTER_DAMAGE: f32 = 60.0;
/// Retail PrimaryDamageRadius residual splash.
pub const BATTLE_MASTER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange.
pub const BATTLE_MASTER_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const BATTLE_MASTER_BASE_DELAY_FRAMES: u32 = 60;
/// Retail WeaponSpeed residual (shell flight residual; host hits still residual-instant).
pub const BATTLE_MASTER_PROJECTILE_SPEED: f32 = 400.0;

/// Uranium PLAYER_UPGRADE WeaponBonus DAMAGE 125%.
pub const BATTLE_MASTER_URANIUM_DAMAGE_MULT: f32 = 1.25;
/// HORDE WeaponBonus RATE_OF_FIRE 150%.
pub const BATTLE_MASTER_HORDE_ROF_MULT: f32 = 1.5;
/// NATIONALISM WeaponBonus RATE_OF_FIRE 125% (stacks with horde when both active).
pub const BATTLE_MASTER_NATIONALISM_ROF_MULT: f32 = 1.25;

/// Retail HordeUpdate Radius (exact-match allies counted within this).
pub const BATTLE_MASTER_HORDE_RADIUS: f32 = 75.0;
/// Retail HordeUpdate Count (includes self via C++ minCount-1 others).
pub const BATTLE_MASTER_HORDE_COUNT: u32 = 5;
/// Retail HordeUpdate UpdateRate 1000ms → 30 frames @ 30 FPS.
pub const BATTLE_MASTER_HORDE_UPDATE_FRAMES: u32 = 30;

/// Residual fire audio.
pub const BATTLE_MASTER_FIRE_AUDIO: &str = "BattlemasterTankWeapon";

/// Whether template is a residual Battlemaster tank chassis.
///
/// Fail-closed: name residual. Excludes weapons/shells/debris/science tokens.
pub fn is_battlemaster_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("training")
        || n.contains("gun")
        || n.contains("crate")
        || n.contains("locomotor")
    {
        return false;
    }
    n.contains("battlemaster")
        || n.contains("battlemastertank")
        || n.contains("tankbattlemaster")
        || n == "china_battlemastertank"
        || n == "china_battletank"
        || n == "testbattlemaster"
}

/// Whether Uranium Shells upgrade tag is present.
pub fn has_uranium_shells_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("uraniumshell") || l == "upgrade_chinauraniumshells"
    })
}

/// Whether Nationalism upgrade tag is present on the unit residual.
pub fn has_nationalism_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("nationalism") || l == "upgrade_nationalism" || l.contains("chinanationalism")
    })
}

/// Apply Uranium residual damage mult when upgrade present.
pub fn battlemaster_damage_with_uranium(base_damage: f32, has_uranium: bool) -> f32 {
    if has_uranium {
        base_damage * BATTLE_MASTER_URANIUM_DAMAGE_MULT
    } else {
        base_damage
    }
}

/// Combined ROF multiplier residual (HORDE * NATIONALISM when both active).
///
/// Nationalism only applies while in horde (C++ AIUpdate evaluateMoraleBonus).
pub fn battlemaster_rof_multiplier(in_horde: bool, has_nationalism: bool) -> f32 {
    let mut rof = 1.0_f32;
    if in_horde {
        rof *= BATTLE_MASTER_HORDE_ROF_MULT;
        if has_nationalism {
            rof *= BATTLE_MASTER_NATIONALISM_ROF_MULT;
        }
    }
    rof
}

/// Delay frames residual: floor(base / ROF), min 1.
pub fn battlemaster_delay_frames(in_horde: bool, has_nationalism: bool) -> u32 {
    let base = BATTLE_MASTER_BASE_DELAY_FRAMES as f32;
    let rof = battlemaster_rof_multiplier(in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for bonuses.
pub fn battlemaster_weapon_stats(
    has_uranium: bool,
    in_horde: bool,
    has_nationalism: bool,
) -> (f32, f32, u32, f32, f32) {
    let dmg = battlemaster_damage_with_uranium(BATTLE_MASTER_DAMAGE, has_uranium);
    let delay = battlemaster_delay_frames(in_horde, has_nationalism);
    (
        dmg,
        BATTLE_MASTER_RANGE,
        delay,
        BATTLE_MASTER_SPLASH_RADIUS,
        BATTLE_MASTER_PROJECTILE_SPEED,
    )
}

/// Build residual Weapon for uranium + horde/nationalism ROF residual.
pub fn battlemaster_weapon(has_uranium: bool, in_horde: bool, has_nationalism: bool) -> Weapon {
    let (damage, range, delay, _splash, speed) =
        battlemaster_weapon_stats(has_uranium, in_horde, has_nationalism);
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
pub fn battlemaster_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= BATTLE_MASTER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_battlemaster_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Battlemaster residual path.
pub fn should_apply_battlemaster_residual(is_battlemaster: bool) -> bool {
    is_battlemaster
}

/// Horde residual: Count includes self (C++: others >= Count-1).
///
/// `nearby_same_type_allies` is the count of *other* exact-match allies within Radius.
pub fn is_in_horde(nearby_same_type_allies: u32) -> bool {
    // nearby others + self >= Count  ⇒ others >= Count - 1
    nearby_same_type_allies + 1 >= BATTLE_MASTER_HORDE_COUNT
}

/// 2D distance residual (XZ plane).
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Whether other unit counts toward self's horde residual (ExactMatch + AlliesOnly).
pub fn counts_toward_battlemaster_horde(
    self_alive: bool,
    other_alive: bool,
    same_team: bool,
    other_is_battlemaster: bool,
    distance: f32,
    radius: f32,
) -> bool {
    self_alive
        && other_alive
        && same_team
        && other_is_battlemaster
        && distance <= radius
        && distance >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn battlemaster_name_matrix() {
        assert!(is_battlemaster_template("ChinaTankBattleMaster"));
        assert!(is_battlemaster_template("China_BattlemasterTank"));
        assert!(is_battlemaster_template("China_BattleTank"));
        assert!(is_battlemaster_template("TestBattlemaster"));
        assert!(is_battlemaster_template("Tank_ChinaTankBattleMaster"));
        assert!(is_battlemaster_template("Nuke_ChinaTankBattleMaster"));
        assert!(!is_battlemaster_template("BattleMasterTankGun"));
        assert!(!is_battlemaster_template("BattleMasterTankShell"));
        assert!(!is_battlemaster_template("SCIENCE_BattlemasterTraining"));
        assert!(!is_battlemaster_template("Upgrade_ChinaUraniumShells"));
        assert!(!is_battlemaster_template("ChinaTankDragon"));
        assert!(!is_battlemaster_template("ChinaTankOverlord"));
        assert!(!is_battlemaster_template("BattleMasterLocomotor"));
    }

    #[test]
    fn base_gun_stats() {
        let (d, r, f, s, sp) = battlemaster_weapon_stats(false, false, false);
        assert!((d - 60.0).abs() < 0.01);
        assert!((r - 150.0).abs() < 0.01);
        assert_eq!(f, 60);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 400.0).abs() < 0.01);
        let w = battlemaster_weapon(false, false, false);
        assert!((w.damage - 60.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.01);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);
    }

    #[test]
    fn uranium_damage_125_percent() {
        let (d, _, f, _, _) = battlemaster_weapon_stats(true, false, false);
        assert!((d - 75.0).abs() < 0.01);
        assert_eq!(f, 60); // uranium is damage, not ROF
        assert!((battlemaster_damage_with_uranium(60.0, true) - 75.0).abs() < 0.01);

        let mut tags = HashSet::new();
        tags.insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        assert!(has_uranium_shells_upgrade(&tags));
        assert!(!has_uranium_shells_upgrade(&HashSet::new()));
    }

    #[test]
    fn horde_and_nationalism_rof_stack() {
        // HORDE alone: floor(60/1.5)=40
        assert_eq!(battlemaster_delay_frames(true, false), 40);
        // Nationalism alone without horde does nothing residual.
        assert_eq!(battlemaster_delay_frames(false, true), 60);
        // HORDE + NATIONALISM: floor(60/1.875)=32
        assert_eq!(battlemaster_delay_frames(true, true), 32);

        let w_horde = battlemaster_weapon(false, true, false);
        let w_both = battlemaster_weapon(false, true, true);
        let w_base = battlemaster_weapon(false, false, false);
        assert!(w_horde.reload_time < w_base.reload_time - 0.05);
        assert!(w_both.reload_time < w_horde.reload_time - 0.01);
        // Uranium + full ROF stack
        let w_full = battlemaster_weapon(true, true, true);
        assert!((w_full.damage - 75.0).abs() < 0.01);
        assert!((w_full.reload_time - (32.0 / 30.0)).abs() < 0.02);
    }

    #[test]
    fn horde_count_includes_self() {
        // 4 others + self = 5 → in horde
        assert!(is_in_horde(4));
        assert!(!is_in_horde(3));
        assert!(!is_in_horde(0));
        assert!(is_in_horde(5));
    }

    #[test]
    fn splash_radius_5() {
        assert!((battlemaster_splash_damage_at(true, 100.0, 60.0) - 60.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 4.0, 60.0) - 60.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 5.0, 75.0) - 75.0).abs() < 0.01);
        assert!((battlemaster_splash_damage_at(false, 5.1, 60.0)).abs() < 0.01);
    }

    #[test]
    fn nationalism_tag_detect() {
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_NATIONALISM.to_string());
        assert!(has_nationalism_upgrade(&tags));
        tags.clear();
        tags.insert("Upgrade_ChinaNationalism".to_string());
        assert!(has_nationalism_upgrade(&tags));
    }

    #[test]
    fn counts_toward_horde_filters() {
        assert!(counts_toward_battlemaster_horde(
            true, true, true, true, 50.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, true, false, true, 50.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, true, true, true, 80.0, 75.0
        ));
        assert!(!counts_toward_battlemaster_horde(
            true, false, true, true, 10.0, 75.0
        ));
    }
}
