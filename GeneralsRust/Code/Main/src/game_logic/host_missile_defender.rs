//! Host USA Missile Defender residual (missile primary + laser guided secondary).
//!
//! Residual slice (playability):
//! - `AmericaInfantryMissileDefender` / USA_ / SupW_ variants spawn with PRIMARY
//!   `MissileDefenderMissileWeapon` (dmg **40** / radius **5** / range **175** /
//!   Delay **1000**ms → 30 frames / AA+ground) and SECONDARY
//!   `MissileDefenderLaserGuidedMissileWeapon` (dmg **40** / radius **5** /
//!   range **300** / Delay **500**ms → 15 frames / AA+ground).
//! - Fire residual: intended + PrimaryDamageRadius **5** splash take full PrimaryDamage.
//! - Laser guided special residual (`SpecialAbilityMissileDefenderLaserGuidedMissiles`):
//!   lock secondary weapon slot + attack target (StartAbilityRange **200** residual).
//!   SpecialPower ReloadTime **0** residual (no host cooldown gate).
//!
//! Wave 60 residual pack (retail INI honesty):
//! - Primary residual: dmg **40**/r**5**/range **175**/Delay **1000**ms → **30**f,
//!   DamageType **INFANTRY_MISSILE**, DeathType **NORMAL**, WeaponSpeed **600**,
//!   FireSound **MissileDefenderWeapon**, FireFX **FX_BuggyMissileIgnition**,
//!   ClipSize **0**, AutoReloadsClip **Yes**, Projectile **MissileDefenderMissile**,
//!   ScatterRadiusVsInfantry **10**, WeaponBonus DAMAGE **125%**.
//! - Laser residual: dmg **40**/r**5**/range **300**/Delay **500**ms → **15**f,
//!   DamageType **ARMOR_PIERCING**, FireSound **MissileDefenderWeapon**,
//!   AutoChooseSources SECONDARY **NONE**.
//! - Laser lock residual: StartAbilityRange **200**, AbilityAbortRange **250**,
//!   PreparationTime **1000**ms → **30**f, PersistentPrepTime **500**ms → **15**f,
//!   SpecialObject **LaserBeam**, ReloadTime **0**, InitiateSound
//!   **MissileDefenderVoiceAttackLaser**.
//! - Body residual: MaxHealth **100**, Vision **150**, Shroud **400**, BuildCost **300**.
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate LaserBeam special object attach-bone matrix
//! - Not full ScatterRadiusVsInfantry random miss matrix
//! - Not full PLAYER_UPGRADE DAMAGE 125% live weapon-bonus apply matrix
//! - Not network laser-lock replication (network deferred)

use super::Weapon;
use crate::game_logic::host_red_guard::delay_frames_to_reload_secs;

/// Logic frames per second (host fixed step).
pub const MD_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const MISSILE_DEFENDER_MISSILE_WEAPON: &str = "MissileDefenderMissileWeapon";
/// Retail secondary laser guided weapon.
pub const MISSILE_DEFENDER_LASER_GUIDED_WEAPON: &str = "MissileDefenderLaserGuidedMissileWeapon";
/// Retail projectile residual.
pub const MISSILE_DEFENDER_MISSILE: &str = "MissileDefenderMissile";
/// Retail special power template.
pub const SPECIAL_ABILITY_MISSILE_DEFENDER_LASER: &str =
    "SpecialAbilityMissileDefenderLaserGuidedMissiles";

/// Retail PrimaryDamage (both weapons).
pub const MISSILE_DEFENDER_DAMAGE: f32 = 40.0;
/// Retail PrimaryDamageRadius residual splash.
pub const MISSILE_DEFENDER_SPLASH_RADIUS: f32 = 5.0;
/// Retail primary AttackRange.
pub const MISSILE_DEFENDER_PRIMARY_RANGE: f32 = 175.0;
/// Retail secondary (laser guided) AttackRange.
pub const MISSILE_DEFENDER_LASER_RANGE: f32 = 300.0;
/// Retail primary DelayBetweenShots residual (msec).
pub const MISSILE_DEFENDER_PRIMARY_DELAY_MS: u32 = 1_000;
/// Retail primary DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES: u32 = 30;
/// Retail secondary DelayBetweenShots residual (msec).
pub const MISSILE_DEFENDER_LASER_DELAY_MS: u32 = 500;
/// Retail secondary DelayBetweenShots 500ms → 15 frames @ 30 FPS.
pub const MISSILE_DEFENDER_LASER_DELAY_FRAMES: u32 = 15;
/// Retail WeaponSpeed residual (missile flight residual; host hits residual-instant).
pub const MISSILE_DEFENDER_PROJECTILE_SPEED: f32 = 600.0;
/// Retail primary DamageType residual.
pub const MISSILE_DEFENDER_PRIMARY_DAMAGE_TYPE: &str = "INFANTRY_MISSILE";
/// Retail laser DamageType residual.
pub const MISSILE_DEFENDER_LASER_DAMAGE_TYPE: &str = "ARMOR_PIERCING";
/// Retail DeathType residual.
pub const MISSILE_DEFENDER_DEATH_TYPE: &str = "NORMAL";
/// Retail ClipSize residual (0 == infinite).
pub const MISSILE_DEFENDER_CLIP_SIZE: u32 = 0;
/// Retail AutoReloadsClip residual.
pub const MISSILE_DEFENDER_AUTO_RELOADS_CLIP: bool = true;
/// Retail ScatterRadiusVsInfantry residual (honesty; host fail-closed no random miss).
pub const MISSILE_DEFENDER_SCATTER_VS_INFANTRY: f32 = 10.0;
/// Retail primary FireFX residual.
pub const MISSILE_DEFENDER_FIRE_FX: &str = "FX_BuggyMissileIgnition";
/// Residual fire audio.
pub const MISSILE_DEFENDER_FIRE_AUDIO: &str = "MissileDefenderWeapon";
/// Retail detonation FX residual.
pub const MISSILE_DEFENDER_DETONATION_FX: &str = "WeaponFX_RocketBuggyMissileDetonation";
/// PLAYER_UPGRADE DAMAGE 125% residual mult honesty (both weapons).
pub const MISSILE_DEFENDER_UPGRADE_DAMAGE_MULT: f32 = 1.25;

// --- Laser lock residual ---

/// SpecialAbilityUpdate StartAbilityRange residual.
pub const LASER_GUIDED_START_ABILITY_RANGE: f32 = 200.0;
/// SpecialAbilityUpdate AbilityAbortRange residual.
pub const LASER_GUIDED_ABORT_RANGE: f32 = 250.0;
/// SpecialPower ReloadTime residual (msec).
pub const LASER_GUIDED_RELOAD_MS: u32 = 0;
/// PreparationTime residual (msec).
pub const LASER_GUIDED_PREP_MS: u32 = 1_000;
/// PreparationTime 1000ms → 30 frames.
pub const LASER_GUIDED_PREP_FRAMES: u32 = 30;
/// PersistentPrepTime residual (msec).
pub const LASER_GUIDED_PERSISTENT_PREP_MS: u32 = 500;
/// PersistentPrepTime 500ms → 15 frames.
pub const LASER_GUIDED_PERSISTENT_PREP_FRAMES: u32 = 15;
/// Retail SpecialObject residual.
pub const LASER_GUIDED_SPECIAL_OBJECT: &str = "LaserBeam";
/// Retail SpecialObjectAttachToBone residual.
pub const LASER_GUIDED_ATTACH_BONE: &str = "Muzzle01";
/// Residual laser special initiate voice.
pub const LASER_GUIDED_INITIATE_AUDIO: &str = "MissileDefenderVoiceAttackLaser";
/// Retail AutoChooseSources SECONDARY residual (NONE = special-only).
pub const LASER_GUIDED_AUTO_CHOOSE_SECONDARY_NONE: bool = true;

// --- Body residual ---

/// Retail MaxHealth residual.
pub const MISSILE_DEFENDER_MAX_HEALTH: f32 = 100.0;
/// Retail VisionRange residual.
pub const MISSILE_DEFENDER_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const MISSILE_DEFENDER_SHROUD_CLEARING_RANGE: f32 = 400.0;
/// Retail BuildCost residual.
pub const MISSILE_DEFENDER_BUILD_COST: u32 = 300;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn md_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * MD_LOGIC_FPS / 1000.0).round() as u32
}

/// Whether template is a residual USA Missile Defender infantry.
///
/// Fail-closed: name residual. Excludes weapons/projectiles/locomotor tokens.
pub fn is_missile_defender_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("exhaust")
        || n.ends_with("missile")
    // MissileDefenderMissile projectile object
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testmissiledefender" || n == "usa_missiledefender" || n == "usa_missile_defender" {
        return true;
    }
    n.contains("missiledefender") || n.contains("missile_defender")
}

/// Apply PLAYER_UPGRADE DAMAGE 125% residual mult honesty.
pub fn missile_defender_damage_with_upgrade(has_upgrade: bool) -> f32 {
    if has_upgrade {
        MISSILE_DEFENDER_DAMAGE * MISSILE_DEFENDER_UPGRADE_DAMAGE_MULT
    } else {
        MISSILE_DEFENDER_DAMAGE
    }
}

/// Build residual primary MissileDefenderMissileWeapon.
pub fn missile_defender_primary_weapon() -> Weapon {
    Weapon {
        damage: MISSILE_DEFENDER_DAMAGE,
        range: MISSILE_DEFENDER_PRIMARY_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: MISSILE_DEFENDER_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Build residual secondary MissileDefenderLaserGuidedMissileWeapon.
pub fn missile_defender_laser_guided_weapon() -> Weapon {
    Weapon {
        damage: MISSILE_DEFENDER_DAMAGE,
        range: MISSILE_DEFENDER_LASER_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(MISSILE_DEFENDER_LASER_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: MISSILE_DEFENDER_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// (damage, range, delay_frames, splash_radius, projectile_speed) for slot.
///
/// Slot 0 = primary, slot 1 = laser guided secondary.
pub fn missile_defender_weapon_stats(slot: u8) -> (f32, f32, u32, f32, f32) {
    if slot == 1 {
        (
            MISSILE_DEFENDER_DAMAGE,
            MISSILE_DEFENDER_LASER_RANGE,
            MISSILE_DEFENDER_LASER_DELAY_FRAMES,
            MISSILE_DEFENDER_SPLASH_RADIUS,
            MISSILE_DEFENDER_PROJECTILE_SPEED,
        )
    } else {
        (
            MISSILE_DEFENDER_DAMAGE,
            MISSILE_DEFENDER_PRIMARY_RANGE,
            MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES,
            MISSILE_DEFENDER_SPLASH_RADIUS,
            MISSILE_DEFENDER_PROJECTILE_SPEED,
        )
    }
}

/// Splash residual damage at distance from impact.
///
/// Intended target takes full PrimaryDamage; others within PrimaryDamageRadius
/// take full PrimaryDamage residual (fail-closed vs continuous falloff).
pub fn missile_defender_splash_damage_at(
    is_intended_target: bool,
    distance_from_impact: f32,
    damage: f32,
) -> f32 {
    if is_intended_target {
        return damage;
    }
    if distance_from_impact <= MISSILE_DEFENDER_SPLASH_RADIUS {
        damage
    } else {
        0.0
    }
}

/// Legal residual splash target.
pub fn is_legal_missile_defender_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Whether residual fire should apply Missile Defender residual path.
pub fn should_apply_missile_defender_residual(is_missile_defender: bool) -> bool {
    is_missile_defender
}

/// Whether unit can issue laser guided special residual.
pub fn can_activate_laser_guided(is_missile_defender: bool, is_alive: bool) -> bool {
    is_missile_defender && is_alive
}

/// Whether target is within StartAbilityRange residual for laser guided special.
pub fn laser_guided_in_start_range(distance: f32) -> bool {
    distance <= LASER_GUIDED_START_ABILITY_RANGE
}

/// Whether target is still inside AbilityAbortRange residual.
pub fn laser_guided_in_abort_range(distance: f32) -> bool {
    distance <= LASER_GUIDED_ABORT_RANGE
}

/// Whether residual fire is laser-guided secondary path (active_weapon_slot == 1).
pub fn is_laser_guided_slot(active_weapon_slot: u8) -> bool {
    active_weapon_slot == 1
}

// --- Wave 60 residual honesty packs ---

/// Wave 60 residual honesty: primary missile residual.
pub fn honesty_missile_defender_primary_residual_ok() -> bool {
    (MISSILE_DEFENDER_DAMAGE - 40.0).abs() < 0.01
        && (MISSILE_DEFENDER_SPLASH_RADIUS - 5.0).abs() < 0.01
        && (MISSILE_DEFENDER_PRIMARY_RANGE - 175.0).abs() < 0.01
        && MISSILE_DEFENDER_PRIMARY_DELAY_MS == 1_000
        && MISSILE_DEFENDER_PRIMARY_DELAY_FRAMES
            == md_ms_to_frames(MISSILE_DEFENDER_PRIMARY_DELAY_MS)
        && (MISSILE_DEFENDER_PROJECTILE_SPEED - 600.0).abs() < 0.01
        && MISSILE_DEFENDER_MISSILE_WEAPON == "MissileDefenderMissileWeapon"
        && MISSILE_DEFENDER_MISSILE == "MissileDefenderMissile"
        && MISSILE_DEFENDER_PRIMARY_DAMAGE_TYPE == "INFANTRY_MISSILE"
        && MISSILE_DEFENDER_DEATH_TYPE == "NORMAL"
        && MISSILE_DEFENDER_CLIP_SIZE == 0
        && MISSILE_DEFENDER_AUTO_RELOADS_CLIP
        && (MISSILE_DEFENDER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && MISSILE_DEFENDER_FIRE_FX == "FX_BuggyMissileIgnition"
        && MISSILE_DEFENDER_FIRE_AUDIO == "MissileDefenderWeapon"
        && MISSILE_DEFENDER_DETONATION_FX == "WeaponFX_RocketBuggyMissileDetonation"
        && (MISSILE_DEFENDER_UPGRADE_DAMAGE_MULT - 1.25).abs() < 0.001
        && (missile_defender_damage_with_upgrade(false) - 40.0).abs() < 0.01
        && (missile_defender_damage_with_upgrade(true) - 50.0).abs() < 0.01
        && {
            let w = missile_defender_primary_weapon();
            (w.damage - 40.0).abs() < 0.01
                && (w.range - 175.0).abs() < 0.01
                && w.can_target_air
                && w.can_target_ground
        }
}

/// Wave 60 residual honesty: laser guided weapon residual.
pub fn honesty_missile_defender_laser_weapon_residual_ok() -> bool {
    MISSILE_DEFENDER_LASER_GUIDED_WEAPON == "MissileDefenderLaserGuidedMissileWeapon"
        && (MISSILE_DEFENDER_LASER_RANGE - 300.0).abs() < 0.01
        && MISSILE_DEFENDER_LASER_DELAY_MS == 500
        && MISSILE_DEFENDER_LASER_DELAY_FRAMES == md_ms_to_frames(MISSILE_DEFENDER_LASER_DELAY_MS)
        && MISSILE_DEFENDER_LASER_DAMAGE_TYPE == "ARMOR_PIERCING"
        && LASER_GUIDED_AUTO_CHOOSE_SECONDARY_NONE
        && {
            let w = missile_defender_laser_guided_weapon();
            (w.damage - 40.0).abs() < 0.01
                && (w.range - 300.0).abs() < 0.01
                && (w.reload_time - 0.5).abs() < 0.01
                && w.can_target_air
                && w.can_target_ground
        }
        && (missile_defender_splash_damage_at(false, 5.0, 40.0) - 40.0).abs() < 0.01
        && missile_defender_splash_damage_at(false, 5.1, 40.0).abs() < 0.01
}

/// Wave 60 residual honesty: laser lock special residual.
pub fn honesty_missile_defender_laser_lock_residual_ok() -> bool {
    SPECIAL_ABILITY_MISSILE_DEFENDER_LASER
        == "SpecialAbilityMissileDefenderLaserGuidedMissiles"
        && (LASER_GUIDED_START_ABILITY_RANGE - 200.0).abs() < 0.01
        && (LASER_GUIDED_ABORT_RANGE - 250.0).abs() < 0.01
        && LASER_GUIDED_RELOAD_MS == 0
        && LASER_GUIDED_PREP_MS == 1_000
        && LASER_GUIDED_PREP_FRAMES == md_ms_to_frames(LASER_GUIDED_PREP_MS)
        && LASER_GUIDED_PERSISTENT_PREP_MS == 500
        && LASER_GUIDED_PERSISTENT_PREP_FRAMES == md_ms_to_frames(LASER_GUIDED_PERSISTENT_PREP_MS)
        && LASER_GUIDED_SPECIAL_OBJECT == "LaserBeam"
        && LASER_GUIDED_ATTACH_BONE == "Muzzle01"
        && LASER_GUIDED_INITIATE_AUDIO == "MissileDefenderVoiceAttackLaser"
        && can_activate_laser_guided(true, true)
        && !can_activate_laser_guided(false, true)
        && laser_guided_in_start_range(200.0)
        && !laser_guided_in_start_range(200.1)
        && laser_guided_in_abort_range(250.0)
        && !laser_guided_in_abort_range(250.1)
        && is_laser_guided_slot(1)
        && !is_laser_guided_slot(0)
        // Abort range exceeds start range residual honesty.
        && LASER_GUIDED_ABORT_RANGE > LASER_GUIDED_START_ABILITY_RANGE
}

/// Wave 60 residual honesty: body residual.
pub fn honesty_missile_defender_body_residual_ok() -> bool {
    (MISSILE_DEFENDER_MAX_HEALTH - 100.0).abs() < 0.01
        && (MISSILE_DEFENDER_VISION_RANGE - 150.0).abs() < 0.01
        && (MISSILE_DEFENDER_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && MISSILE_DEFENDER_BUILD_COST == 300
}

/// Combined Wave 60 Missile Defender residual honesty pack.
pub fn honesty_missile_defender_residual_pack_ok() -> bool {
    honesty_missile_defender_primary_residual_ok()
        && honesty_missile_defender_laser_weapon_residual_ok()
        && honesty_missile_defender_laser_lock_residual_ok()
        && honesty_missile_defender_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missile_defender_name_matrix() {
        assert!(is_missile_defender_template(
            "AmericaInfantryMissileDefender"
        ));
        assert!(is_missile_defender_template("USA_MissileDefender"));
        assert!(is_missile_defender_template(
            "SupW_AmericaInfantryMissileDefender"
        ));
        assert!(is_missile_defender_template("TestMissileDefender"));
        assert!(!is_missile_defender_template(
            "MissileDefenderMissileWeapon"
        ));
        assert!(!is_missile_defender_template(
            "MissileDefenderLaserGuidedMissileWeapon"
        ));
        assert!(!is_missile_defender_template("MissileDefenderMissile"));
        assert!(!is_missile_defender_template("MissileDefenderLocomotor"));
        assert!(!is_missile_defender_template("AmericaInfantryRanger"));
        assert!(!is_missile_defender_template("ChinaInfantryTankHunter"));
        assert!(!is_missile_defender_template("GLAInfantryTunnelDefender"));
    }

    #[test]
    fn primary_and_laser_stats() {
        let (d, r, f, s, sp) = missile_defender_weapon_stats(0);
        assert!((d - 40.0).abs() < 0.01);
        assert!((r - 175.0).abs() < 0.01);
        assert_eq!(f, 30);
        assert!((s - 5.0).abs() < 0.01);
        assert!((sp - 600.0).abs() < 0.01);
        let w = missile_defender_primary_weapon();
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.reload_time - 1.0).abs() < 0.01);
        assert!(w.can_target_air && w.can_target_ground);

        let (d2, r2, f2, _, _) = missile_defender_weapon_stats(1);
        assert!((d2 - 40.0).abs() < 0.01);
        assert!((r2 - 300.0).abs() < 0.01);
        assert_eq!(f2, 15);
        let lw = missile_defender_laser_guided_weapon();
        assert!((lw.range - 300.0).abs() < 0.01);
        assert!((lw.reload_time - 0.5).abs() < 0.01);
        assert!(lw.can_target_air && lw.can_target_ground);
    }

    #[test]
    fn splash_residual() {
        assert!((missile_defender_splash_damage_at(true, 100.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 4.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 5.0, 40.0) - 40.0).abs() < 0.01);
        assert!((missile_defender_splash_damage_at(false, 5.1, 40.0)).abs() < 0.01);
    }

    #[test]
    fn laser_special_gate() {
        assert!(can_activate_laser_guided(true, true));
        assert!(!can_activate_laser_guided(false, true));
        assert!(!can_activate_laser_guided(true, false));
        assert!(laser_guided_in_start_range(200.0));
        assert!(!laser_guided_in_start_range(200.1));
        assert!(is_laser_guided_slot(1));
        assert!(!is_laser_guided_slot(0));
        assert!(should_apply_missile_defender_residual(true));
        assert!(!should_apply_missile_defender_residual(false));
    }

    #[test]
    fn missile_defender_residual_pack_honesty() {
        assert!(honesty_missile_defender_primary_residual_ok());
        assert!(honesty_missile_defender_laser_weapon_residual_ok());
        assert!(honesty_missile_defender_laser_lock_residual_ok());
        assert!(honesty_missile_defender_body_residual_ok());
        assert!(honesty_missile_defender_residual_pack_ok());
        assert_eq!(md_ms_to_frames(1_000), 30);
        assert_eq!(md_ms_to_frames(500), 15);
        assert_eq!(md_ms_to_frames(0), 0);
    }
}
