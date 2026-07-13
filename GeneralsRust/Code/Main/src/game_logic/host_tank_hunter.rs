//! Host China Tank Hunter residual (RPG missile + TNT special + horde/nationalism ROF).
//!
//! Residual slice (playability):
//! - `ChinaInfantryTankHunter` / variants spawn with PRIMARY
//!   `ChinaInfantryTankHunterMissileLauncher` (dmg **40** / radius **5** /
//!   range **175** / min **5** / Delay **1000**ms → 30 frames / AA+ground).
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//! - Horde residual (same China infantry HordeUpdate as Red Guard: KindOf INFANTRY,
//!   Radius **30**, Count **5**): RATE_OF_FIRE **150%** → floor(30/1.5)=**20** frames.
//! - Nationalism residual while in horde: additional ROF **125%** → **16** frames.
//! - TNT special residual (`SpecialAbilityTankHunterTNTAttack`): plant sticky
//!   timed charge (`TNTStickyBomb` / TNTDetonationWeapon 500/10 + 150/50) with
//!   ReloadTime **7500**ms residual cooldown and StartAbilityRange **5**.
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate flee-after / MaxSpecialObjects=8 list / attach bones
//! - Not full ScatterRadiusVsInfantry / projectile exhaust FX matrix
//! - Not full HordeUpdate RubOffRadius honorary-member matrix
//! - Not Fanaticism infantry-general nationalism branch
//! - Not network TNT / RPG replication (network deferred)

use super::Weapon;
use crate::game_logic::host_battlemaster::has_nationalism_upgrade;
use crate::game_logic::host_red_guard::{
    delay_frames_to_reload_secs, is_in_infantry_horde, INFANTRY_HORDE_ROF_MULT,
    INFANTRY_NATIONALISM_ROF_MULT,
};

/// Retail primary weapon.
pub const TANK_HUNTER_MISSILE_WEAPON: &str = "ChinaInfantryTankHunterMissileLauncher";
/// Retail TNT detonation weapon (charge payload).
pub const TNT_DETONATION_WEAPON: &str = "TNTDetonationWeapon";
/// Retail sticky bomb object.
pub const TNT_STICKY_BOMB: &str = "TNTStickyBomb";
/// Retail special power template name.
pub const SPECIAL_ABILITY_TANK_HUNTER_TNT: &str = "SpecialAbilityTankHunterTNTAttack";

/// Retail PrimaryDamage.
pub const TANK_HUNTER_DAMAGE: f32 = 40.0;
/// Retail PrimaryDamageRadius residual splash.
pub const TANK_HUNTER_SPLASH_RADIUS: f32 = 5.0;
/// Retail AttackRange.
pub const TANK_HUNTER_RANGE: f32 = 175.0;
/// Retail MinimumAttackRange.
pub const TANK_HUNTER_MIN_RANGE: f32 = 5.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const TANK_HUNTER_BASE_DELAY_FRAMES: u32 = 30;
/// Retail WeaponSpeed residual (missile flight residual; host hits still residual-instant).
pub const TANK_HUNTER_PROJECTILE_SPEED: f32 = 600.0;

/// TNT special: SpecialPower ReloadTime 7500ms → 225 frames @ 30 FPS.
pub const TNT_RELOAD_FRAMES: u32 = 225;
/// SpecialAbilityUpdate StartAbilityRange residual.
pub const TNT_START_ABILITY_RANGE: f32 = 5.0;
/// TNTDetonationWeapon PrimaryDamage residual.
pub const TNT_PRIMARY_DAMAGE: f32 = 500.0;
/// TNTDetonationWeapon PrimaryDamageRadius residual.
pub const TNT_PRIMARY_RADIUS: f32 = 10.0;
/// TNTDetonationWeapon SecondaryDamage residual.
pub const TNT_SECONDARY_DAMAGE: f32 = 150.0;
/// TNTDetonationWeapon SecondaryDamageRadius residual.
pub const TNT_SECONDARY_RADIUS: f32 = 50.0;
/// TNTStickyBomb LifetimeUpdate 10000ms → 300 frames (matches host_mines TimedDemoCharge).
pub const TNT_LIFETIME_FRAMES: u32 = 300;

/// Residual fire audio.
pub const TANK_HUNTER_FIRE_AUDIO: &str = "TankHunterWeapon";
/// Residual TNT initiate voice.
pub const TNT_INITIATE_AUDIO: &str = "TankHunterVoiceTNT";

/// Whether template is a residual Tank Hunter infantry.
///
/// Fail-closed: name residual. Excludes weapons/debris/science tokens.
pub fn is_tank_hunter_template(template_name: &str) -> bool {
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
        || n.contains("stickybomb")
        || n.contains("sticky_bomb")
        || n.contains("detonation")
    {
        return false;
    }
    n.contains("tankhunter")
        || n.contains("tank_hunter")
        || n == "testtankhunter"
}

/// Combined ROF multiplier residual (HORDE * NATIONALISM when both active).
pub fn tank_hunter_rof_multiplier(in_horde: bool, has_nationalism: bool) -> f32 {
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
pub fn tank_hunter_delay_frames(in_horde: bool, has_nationalism: bool) -> u32 {
    let base = TANK_HUNTER_BASE_DELAY_FRAMES as f32;
    let rof = tank_hunter_rof_multiplier(in_horde, has_nationalism);
    (base / rof).floor().max(1.0) as u32
}

/// (damage, range, min_range, delay_frames, splash_radius, projectile_speed).
pub fn tank_hunter_weapon_stats(
    in_horde: bool,
    has_nationalism: bool,
) -> (f32, f32, f32, u32, f32, f32) {
    let delay = tank_hunter_delay_frames(in_horde, has_nationalism);
    (
        TANK_HUNTER_DAMAGE,
        TANK_HUNTER_RANGE,
        TANK_HUNTER_MIN_RANGE,
        delay,
        TANK_HUNTER_SPLASH_RADIUS,
        TANK_HUNTER_PROJECTILE_SPEED,
    )
}

/// Build residual RPG Weapon with horde/nationalism ROF residual.
pub fn tank_hunter_weapon(in_horde: bool, has_nationalism: bool) -> Weapon {
    let (damage, range, min_range, delay, _splash, speed) =
        tank_hunter_weapon_stats(in_horde, has_nationalism);
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
pub fn tank_hunter_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= TANK_HUNTER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_tank_hunter_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Tank Hunter RPG residual path.
pub fn should_apply_tank_hunter_residual(is_tank_hunter: bool) -> bool {
    is_tank_hunter
}

/// Whether unit can issue TNT special residual (Tank Hunter only).
pub fn can_plant_tank_hunter_tnt(is_tank_hunter: bool, is_alive: bool, can_move: bool) -> bool {
    is_tank_hunter && is_alive && can_move
}

/// Whether TNT special is ready (cooldown residual).
pub fn tnt_ready(current_frame: u32, last_tnt_frame: Option<u32>) -> bool {
    match last_tnt_frame {
        None => true,
        Some(last) => current_frame.saturating_sub(last) >= TNT_RELOAD_FRAMES,
    }
}

/// Whether target is legal for TNT residual (structure or ground vehicle).
pub fn is_legal_tnt_target(
    target_alive: bool,
    target_is_structure: bool,
    target_is_vehicle: bool,
    target_is_airborne: bool,
    same_team: bool,
) -> bool {
    target_alive
        && !same_team
        && (target_is_structure || (target_is_vehicle && !target_is_airborne))
}

/// Nationalism tag detect residual.
pub fn tank_hunter_has_nationalism(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    has_nationalism_upgrade(applied_upgrades)
}

/// Re-export horde helper for tests.
pub fn tank_hunter_is_in_horde(nearby: u32) -> bool {
    is_in_infantry_horde(nearby)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn tank_hunter_name_matrix() {
        assert!(is_tank_hunter_template("ChinaInfantryTankHunter"));
        assert!(is_tank_hunter_template("China_TankHunter"));
        assert!(is_tank_hunter_template("Tank_ChinaInfantryTankHunter"));
        assert!(is_tank_hunter_template("Nuke_ChinaInfantryTankHunter"));
        assert!(is_tank_hunter_template("Infa_ChinaInfantryTankHunter"));
        assert!(is_tank_hunter_template("TestTankHunter"));
        assert!(!is_tank_hunter_template("ChinaInfantryTankHunterMissileLauncher"));
        assert!(!is_tank_hunter_template("TankHunterMissile"));
        assert!(!is_tank_hunter_template("TNTStickyBomb"));
        assert!(!is_tank_hunter_template("TNTDetonationWeapon"));
        assert!(!is_tank_hunter_template("ChinaInfantryRedguard"));
        assert!(!is_tank_hunter_template("TankHunterMissileLocomotor"));
    }

    #[test]
    fn base_rpg_stats() {
        let (d, r, min_r, f, s, sp) = tank_hunter_weapon_stats(false, false);
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 175.0).abs() < 0.01);
        assert!((min_r - 5.0).abs() < 0.01);
        assert_eq!(f, 30);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 600.0).abs() < 0.01);
        let w = tank_hunter_weapon(false, false);
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn horde_and_nationalism_rof_stack() {
        assert_eq!(tank_hunter_delay_frames(true, false), 20);
        assert_eq!(tank_hunter_delay_frames(false, true), 30);
        assert_eq!(tank_hunter_delay_frames(true, true), 16);
        let w_horde = tank_hunter_weapon(true, false);
        let w_both = tank_hunter_weapon(true, true);
        let w_base = tank_hunter_weapon(false, false);
        assert!(w_horde.reload_time < w_base.reload_time - 0.05);
        assert!(w_both.reload_time < w_horde.reload_time - 0.01);
    }

    #[test]
    fn splash_radius_5() {
        assert!((tank_hunter_splash_damage_at(true, 100.0, 40.0) - 40.0).abs() < 0.01);
        assert!((tank_hunter_splash_damage_at(false, 4.0, 40.0) - 40.0).abs() < 0.01);
        assert!((tank_hunter_splash_damage_at(false, 5.0, 40.0) - 40.0).abs() < 0.01);
        assert!((tank_hunter_splash_damage_at(false, 5.1, 40.0)).abs() < 0.01);
    }

    #[test]
    fn tnt_cooldown_and_target() {
        assert!(tnt_ready(0, None));
        assert!(!tnt_ready(100, Some(0)));
        assert!(tnt_ready(225, Some(0)));
        assert!(tnt_ready(300, Some(50)));
        assert!(is_legal_tnt_target(true, true, false, false, false));
        assert!(is_legal_tnt_target(true, false, true, false, false));
        assert!(!is_legal_tnt_target(true, false, true, true, false));
        assert!(!is_legal_tnt_target(true, true, false, false, true));
        assert!(!is_legal_tnt_target(false, true, false, false, false));
        assert_eq!(TNT_RELOAD_FRAMES, 225);
        assert!((TNT_START_ABILITY_RANGE - 5.0).abs() < 0.01);
    }

    #[test]
    fn nationalism_tag() {
        let mut tags = HashSet::new();
        tags.insert("Upgrade_Nationalism".to_string());
        assert!(tank_hunter_has_nationalism(&tags));
        assert!(tank_hunter_is_in_horde(4));
        assert!(!tank_hunter_is_in_horde(3));
    }
}
