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
//! Wave 67 residual pack (retail ChinaInfantry.ini / Weapon.ini / Locomotor.ini):
//! - Weapon residual: DamageType **INFANTRY_MISSILE**, DeathType **EXPLODED**,
//!   ScatterRadiusVsInfantry **10**, Projectile **TankHunterMissile**,
//!   FireFX **FX_BuggyMissileIgnition**, DetonationFX **WeaponFX_RocketBuggyMissileDetonation**,
//!   Delay **1000**ms → **30**f, ClipSize **0**, AutoReloadsClip **Yes**.
//! - TNT residual: DamageType **EXPLOSION**, FireSound **BombTruckDefaultBombDetonation**,
//!   Reload **7500**ms → **225**f, Lifetime **10000**ms → **300**f.
//! - Body residual: MaxHealth **100**, Vision **150**, Shroud **400**,
//!   BuildCost **300**, BuildTime **5**s → **150**f, TransportSlotCount **1**,
//!   Locomotor Speed **20**/Damaged **10**, Geometry CYLINDER r**10**/h**12**.
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

/// Logic frames per second (host fixed step).
pub const TANK_HUNTER_LOGIC_FPS: f32 = 30.0;

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
/// Retail DelayBetweenShots residual (msec).
pub const TANK_HUNTER_BASE_DELAY_MS: u32 = 1_000;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const TANK_HUNTER_BASE_DELAY_FRAMES: u32 = 30;
/// Retail WeaponSpeed residual (missile flight residual; host hits still residual-instant).
pub const TANK_HUNTER_PROJECTILE_SPEED: f32 = 600.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const TANK_HUNTER_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail DamageType residual.
pub const TANK_HUNTER_DAMAGE_TYPE: &str = "INFANTRY_MISSILE";
/// Retail DeathType residual.
pub const TANK_HUNTER_DEATH_TYPE: &str = "EXPLODED";
/// Retail ProjectileObject residual.
pub const TANK_HUNTER_PROJECTILE: &str = "TankHunterMissile";
/// Retail FireFX residual.
pub const TANK_HUNTER_FIRE_FX: &str = "FX_BuggyMissileIgnition";
/// Retail ProjectileDetonationFX residual.
pub const TANK_HUNTER_DETONATION_FX: &str = "WeaponFX_RocketBuggyMissileDetonation";
/// Retail ClipSize residual (0 == infinite).
pub const TANK_HUNTER_CLIP_SIZE: u32 = 0;
/// Retail AutoReloadsClip residual.
pub const TANK_HUNTER_AUTO_RELOADS_CLIP: bool = true;

/// TNT special: SpecialPower ReloadTime residual (msec).
pub const TNT_RELOAD_MS: u32 = 7_500;
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
/// TNTDetonationWeapon DamageType residual.
pub const TNT_DAMAGE_TYPE: &str = "EXPLOSION";
/// TNTDetonationWeapon DeathType residual.
pub const TNT_DEATH_TYPE: &str = "EXPLODED";
/// TNTDetonationWeapon FireSound residual.
pub const TNT_FIRE_AUDIO: &str = "BombTruckDefaultBombDetonation";
/// TNTStickyBomb LifetimeUpdate residual (msec).
pub const TNT_LIFETIME_MS: u32 = 10_000;
/// TNTStickyBomb LifetimeUpdate 10000ms → 300 frames (matches host_mines TimedDemoCharge).
pub const TNT_LIFETIME_FRAMES: u32 = 300;

/// Residual fire audio.
pub const TANK_HUNTER_FIRE_AUDIO: &str = "TankHunterWeapon";
/// Residual TNT initiate voice.
pub const TNT_INITIATE_AUDIO: &str = "TankHunterVoiceTNT";

// --- Body residual (ChinaInfantryTankHunter) ---

/// Retail MaxHealth residual.
pub const TANK_HUNTER_MAX_HEALTH: f32 = 100.0;
/// Retail VisionRange residual.
pub const TANK_HUNTER_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const TANK_HUNTER_SHROUD_CLEARING_RANGE: f32 = 400.0;
/// Retail BuildCost residual.
pub const TANK_HUNTER_BUILD_COST: u32 = 300;
/// Retail BuildTime residual (seconds).
pub const TANK_HUNTER_BUILD_TIME_SEC: f32 = 5.0;
/// BuildTime 5s → 150 frames @ 30 FPS.
pub const TANK_HUNTER_BUILD_TIME_FRAMES: u32 = 150;
/// Retail TransportSlotCount residual.
pub const TANK_HUNTER_TRANSPORT_SLOT_COUNT: u32 = 1;
/// Retail MissileDefenderLocomotor Speed residual.
pub const TANK_HUNTER_LOCOMOTOR_SPEED: f32 = 20.0;
/// Retail MissileDefenderLocomotor SpeedDamaged residual.
pub const TANK_HUNTER_LOCOMOTOR_SPEED_DAMAGED: f32 = 10.0;
/// Retail Geometry CYLINDER MajorRadius residual.
pub const TANK_HUNTER_GEOMETRY_RADIUS: f32 = 10.0;
/// Retail GeometryHeight residual.
pub const TANK_HUNTER_GEOMETRY_HEIGHT: f32 = 12.0;
/// Retail ExperienceValue residual.
pub const TANK_HUNTER_EXPERIENCE_VALUE: [u32; 4] = [20, 20, 40, 60];
/// Retail ExperienceRequired residual.
pub const TANK_HUNTER_EXPERIENCE_REQUIRED: [u32; 4] = [0, 100, 200, 400];

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn tank_hunter_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * TANK_HUNTER_LOGIC_FPS / 1000.0).round() as u32
}

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
    n.contains("tankhunter") || n.contains("tank_hunter") || n == "testtankhunter"
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
    let (damage, range, min_range, delay, splash, speed) =
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
        splash_radius: splash,
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

// --- Wave 67 residual honesty packs ---

/// Wave 67 residual honesty: Tank Hunter RPG weapon residual peel.
pub fn honesty_tank_hunter_weapon_residual_ok() -> bool {
    TANK_HUNTER_MISSILE_WEAPON == "ChinaInfantryTankHunterMissileLauncher"
        && (TANK_HUNTER_DAMAGE - 40.0).abs() < 0.01
        && (TANK_HUNTER_SPLASH_RADIUS - 5.0).abs() < 0.01
        && (TANK_HUNTER_RANGE - 175.0).abs() < 0.01
        && (TANK_HUNTER_MIN_RANGE - 5.0).abs() < 0.01
        && TANK_HUNTER_BASE_DELAY_MS == 1_000
        && TANK_HUNTER_BASE_DELAY_FRAMES == tank_hunter_ms_to_frames(TANK_HUNTER_BASE_DELAY_MS)
        && TANK_HUNTER_BASE_DELAY_FRAMES == 30
        && (TANK_HUNTER_PROJECTILE_SPEED - 600.0).abs() < 0.01
        && (TANK_HUNTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && TANK_HUNTER_DAMAGE_TYPE == "INFANTRY_MISSILE"
        && TANK_HUNTER_DEATH_TYPE == "EXPLODED"
        && TANK_HUNTER_PROJECTILE == "TankHunterMissile"
        && TANK_HUNTER_FIRE_FX == "FX_BuggyMissileIgnition"
        && TANK_HUNTER_DETONATION_FX == "WeaponFX_RocketBuggyMissileDetonation"
        && TANK_HUNTER_CLIP_SIZE == 0
        && TANK_HUNTER_AUTO_RELOADS_CLIP
        && TANK_HUNTER_FIRE_AUDIO == "TankHunterWeapon"
        && {
            let w = tank_hunter_weapon(false, false);
            (w.damage - 40.0).abs() < 0.01 && w.can_target_air && w.can_target_ground
        }
}

/// Wave 67 residual honesty: TNT special residual peel.
pub fn honesty_tank_hunter_tnt_residual_ok() -> bool {
    TNT_DETONATION_WEAPON == "TNTDetonationWeapon"
        && TNT_STICKY_BOMB == "TNTStickyBomb"
        && SPECIAL_ABILITY_TANK_HUNTER_TNT == "SpecialAbilityTankHunterTNTAttack"
        && TNT_RELOAD_MS == 7_500
        && TNT_RELOAD_FRAMES == tank_hunter_ms_to_frames(TNT_RELOAD_MS)
        && TNT_RELOAD_FRAMES == 225
        && (TNT_START_ABILITY_RANGE - 5.0).abs() < 0.01
        && (TNT_PRIMARY_DAMAGE - 500.0).abs() < 0.01
        && (TNT_PRIMARY_RADIUS - 10.0).abs() < 0.01
        && (TNT_SECONDARY_DAMAGE - 150.0).abs() < 0.01
        && (TNT_SECONDARY_RADIUS - 50.0).abs() < 0.01
        && TNT_DAMAGE_TYPE == "EXPLOSION"
        && TNT_DEATH_TYPE == "EXPLODED"
        && TNT_FIRE_AUDIO == "BombTruckDefaultBombDetonation"
        && TNT_LIFETIME_MS == 10_000
        && TNT_LIFETIME_FRAMES == tank_hunter_ms_to_frames(TNT_LIFETIME_MS)
        && TNT_LIFETIME_FRAMES == 300
        && tnt_ready(0, None)
        && tnt_ready(225, Some(0))
        && !tnt_ready(100, Some(0))
}

/// Wave 67 residual honesty: Tank Hunter body residual peel.
pub fn honesty_tank_hunter_body_residual_ok() -> bool {
    (TANK_HUNTER_MAX_HEALTH - 100.0).abs() < 0.01
        && (TANK_HUNTER_VISION_RANGE - 150.0).abs() < 0.01
        && (TANK_HUNTER_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && TANK_HUNTER_BUILD_COST == 300
        && (TANK_HUNTER_BUILD_TIME_SEC - 5.0).abs() < 0.01
        && TANK_HUNTER_BUILD_TIME_FRAMES
            == (TANK_HUNTER_BUILD_TIME_SEC * TANK_HUNTER_LOGIC_FPS).round() as u32
        && TANK_HUNTER_BUILD_TIME_FRAMES == 150
        && TANK_HUNTER_TRANSPORT_SLOT_COUNT == 1
        && (TANK_HUNTER_LOCOMOTOR_SPEED - 20.0).abs() < 0.01
        && (TANK_HUNTER_LOCOMOTOR_SPEED_DAMAGED - 10.0).abs() < 0.01
        && (TANK_HUNTER_GEOMETRY_RADIUS - 10.0).abs() < 0.01
        && (TANK_HUNTER_GEOMETRY_HEIGHT - 12.0).abs() < 0.01
        && TANK_HUNTER_EXPERIENCE_VALUE == [20, 20, 40, 60]
        && TANK_HUNTER_EXPERIENCE_REQUIRED == [0, 100, 200, 400]
        && tank_hunter_delay_frames(true, false) == 20
        && tank_hunter_delay_frames(true, true) == 16
}

/// Combined Wave 67 Tank Hunter residual honesty pack.
pub fn honesty_tank_hunter_residual_pack_ok() -> bool {
    honesty_tank_hunter_weapon_residual_ok()
        && honesty_tank_hunter_tnt_residual_ok()
        && honesty_tank_hunter_body_residual_ok()
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
        assert!(!is_tank_hunter_template(
            "ChinaInfantryTankHunterMissileLauncher"
        ));
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

    #[test]
    fn tank_hunter_residual_pack_honesty_wave67() {
        assert!(honesty_tank_hunter_weapon_residual_ok());
        assert!(honesty_tank_hunter_tnt_residual_ok());
        assert!(honesty_tank_hunter_body_residual_ok());
        assert!(honesty_tank_hunter_residual_pack_ok());
        assert_eq!(tank_hunter_ms_to_frames(1_000), 30);
        assert_eq!(tank_hunter_ms_to_frames(7_500), 225);
        assert_eq!(TANK_HUNTER_BUILD_TIME_FRAMES, 150);
        assert_eq!(TANK_HUNTER_PROJECTILE, "TankHunterMissile");
        assert_eq!(TNT_FIRE_AUDIO, "BombTruckDefaultBombDetonation");
        assert!(TANK_HUNTER_AUTO_RELOADS_CLIP);
    }
}
