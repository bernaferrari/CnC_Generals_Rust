//! Host America Fire Base residual (howitzer dual-radius structure defense).
//!
//! Residual slice (playability):
//! - `AmericaFireBase` / AirF_/SupW_/Lazr_ variants spawn with PRIMARY
//!   `FireBaseHowitzerGun` (PrimaryDamage **75** / radius **10**, range **275**,
//!   min **50**, Delay **2000**ms → 60 frames).
//! - Fire residual: intended + PrimaryDamageRadius **10** splash.
//! - Structure residual: CAN_ATTACK base-defense howitzer (not full spawn matrix).
//!
//! Wave 63 residual pack (retail INI honesty):
//! - Weapon residual: PrimaryDamage **75**/r**10**, AttackRange **275**/min **50**,
//!   Delay **2000**ms → **60**f, ScatterRadiusVsInfantry **15**, DamageType **EXPLOSION**,
//!   DeathType **NORMAL**, WeaponSpeed **300**, MinWeaponSpeed **75**, ScaleWeaponSpeed **Yes**,
//!   Projectile **GenericTankShell**, FireFX **WeaponFX_GenericTankGunNoTracer**,
//!   DetonationFX **FX_FireBaseHowitzerExplosion**, ClipSize **0**.
//! - Body residual: MaxHealth **1000**, Vision/Shroud **360**, BuildCost **1000**,
//!   BuildTime **25**s → **750**f, EnergyProduction **0**.
//!
//! Fail-closed honesty:
//! - Not full SPAWNS_ARE_THE_WEAPONS / garrison-howitzer HiveStructureBody matrix
//! - Not full Turret pitch / ScaleWeaponSpeed lob projectile matrix
//! - Not full ScatterRadiusVsInfantry residual miss cone
//! - Not network Fire Base replication (network deferred)

use super::Weapon;

/// Logic frames per second (host fixed step).
pub const FIRE_BASE_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const FIRE_BASE_HOWITZER_WEAPON: &str = "FireBaseHowitzerGun";

/// Retail PrimaryDamage.
pub const FIRE_BASE_DAMAGE: f32 = 75.0;
/// Retail PrimaryDamageRadius.
pub const FIRE_BASE_PRIMARY_RADIUS: f32 = 10.0;
/// Retail AttackRange.
pub const FIRE_BASE_RANGE: f32 = 275.0;
/// Retail MinimumAttackRange.
pub const FIRE_BASE_MIN_RANGE: f32 = 50.0;
/// Retail DelayBetweenShots residual (msec).
pub const FIRE_BASE_DELAY_MS: u32 = 2_000;
/// Retail DelayBetweenShots 2000ms → 60 frames @ 30 FPS.
pub const FIRE_BASE_DELAY_FRAMES: u32 = 60;
/// Residual projectile speed (ScaleWeaponSpeed lob residual honesty).
pub const FIRE_BASE_PROJECTILE_SPEED: f32 = 300.0;
/// Retail MinWeaponSpeed residual.
pub const FIRE_BASE_MIN_WEAPON_SPEED: f32 = 75.0;
/// Retail ScaleWeaponSpeed residual honesty.
pub const FIRE_BASE_SCALE_WEAPON_SPEED: bool = true;
/// Retail ScatterRadiusVsInfantry residual (honesty only; host fail-closed no random miss).
pub const FIRE_BASE_SCATTER_VS_INFANTRY: f32 = 15.0;
/// Retail DamageType residual.
pub const FIRE_BASE_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const FIRE_BASE_DEATH_TYPE: &str = "NORMAL";
/// Retail ClipSize residual (0 == infinite).
pub const FIRE_BASE_CLIP_SIZE: u32 = 0;
/// Retail ProjectileObject residual.
pub const FIRE_BASE_PROJECTILE: &str = "GenericTankShell";
/// Retail FireFX residual.
pub const FIRE_BASE_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracer";
/// Retail ProjectileDetonationFX residual.
pub const FIRE_BASE_DETONATION_FX: &str = "FX_FireBaseHowitzerExplosion";
/// Residual fire audio.
pub const FIRE_BASE_FIRE_AUDIO: &str = "StrategyCenter_ArtilleryRound";

// --- Body residual (AmericaFireBase) ---

/// Retail MaxHealth residual.
pub const FIRE_BASE_MAX_HEALTH: f32 = 1_000.0;
/// Retail VisionRange residual.
pub const FIRE_BASE_VISION_RANGE: f32 = 360.0;
/// Retail ShroudClearingRange residual.
pub const FIRE_BASE_SHROUD_CLEARING_RANGE: f32 = 360.0;
/// Retail BuildCost residual.
pub const FIRE_BASE_BUILD_COST: u32 = 1_000;
/// Retail BuildTime residual (seconds).
pub const FIRE_BASE_BUILD_TIME_SEC: f32 = 25.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const FIRE_BASE_BUILD_TIME_FRAMES: u32 = 750;
/// Retail EnergyProduction residual.
pub const FIRE_BASE_ENERGY_PRODUCTION: i32 = 0;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn fire_base_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * FIRE_BASE_LOGIC_FPS / 1000.0).round() as u32
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual America Fire Base structure.
///
/// Fail-closed: name residual. Excludes howitzer projectiles / armor tokens / hulks.
pub fn is_fire_base_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "americafirebase"
        || n == "usa_firebase"
        || n == "testfirebase"
        || n == "airfamericafirebase"
        || n == "supwamericafirebase"
        || n == "lazramericafirebase"
    {
        return true;
    }
    // Exclude non-structure residual objects.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.contains("armor")
        || n.contains("howitzer")
        || n.starts_with("upgrade")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("explosion")
    {
        return false;
    }
    n.contains("firebase")
}

/// Whether residual fire should apply Fire Base residual path.
pub fn should_apply_fire_base_residual(is_fire_base: bool) -> bool {
    is_fire_base
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Fire Base howitzer Weapon.
pub fn fire_base_weapon() -> Weapon {
    Weapon {
        damage: FIRE_BASE_DAMAGE,
        range: FIRE_BASE_RANGE,
        min_range: FIRE_BASE_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(FIRE_BASE_DELAY_FRAMES),
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: FIRE_BASE_PROJECTILE_SPEED,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Residual damage at distance from impact (intended / primary ring).
pub fn fire_base_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= FIRE_BASE_PRIMARY_RADIUS {
        FIRE_BASE_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash / fire target.
pub fn is_legal_fire_base_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 63 residual honesty packs ---

/// Wave 63 residual honesty: Fire Base howitzer weapon residual peel.
pub fn honesty_fire_base_weapon_residual_ok() -> bool {
    FIRE_BASE_HOWITZER_WEAPON == "FireBaseHowitzerGun"
        && (FIRE_BASE_DAMAGE - 75.0).abs() < 0.01
        && (FIRE_BASE_PRIMARY_RADIUS - 10.0).abs() < 0.01
        && (FIRE_BASE_RANGE - 275.0).abs() < 0.01
        && (FIRE_BASE_MIN_RANGE - 50.0).abs() < 0.01
        && FIRE_BASE_DELAY_MS == 2_000
        && FIRE_BASE_DELAY_FRAMES == fire_base_ms_to_frames(FIRE_BASE_DELAY_MS)
        && FIRE_BASE_DELAY_FRAMES == 60
        && (FIRE_BASE_PROJECTILE_SPEED - 300.0).abs() < 0.01
        && (FIRE_BASE_MIN_WEAPON_SPEED - 75.0).abs() < 0.01
        && FIRE_BASE_SCALE_WEAPON_SPEED
        && (FIRE_BASE_SCATTER_VS_INFANTRY - 15.0).abs() < 0.01
        && FIRE_BASE_DAMAGE_TYPE == "EXPLOSION"
        && FIRE_BASE_DEATH_TYPE == "NORMAL"
        && FIRE_BASE_CLIP_SIZE == 0
        && FIRE_BASE_PROJECTILE == "GenericTankShell"
        && FIRE_BASE_FIRE_FX == "WeaponFX_GenericTankGunNoTracer"
        && FIRE_BASE_DETONATION_FX == "FX_FireBaseHowitzerExplosion"
        && FIRE_BASE_FIRE_AUDIO == "StrategyCenter_ArtilleryRound"
        && (fire_base_damage_at(0.0) - 75.0).abs() < 0.01
        && (fire_base_damage_at(10.0) - 75.0).abs() < 0.01
        && fire_base_damage_at(10.1).abs() < 0.01
        && {
            let w = fire_base_weapon();
            (w.damage - 75.0).abs() < 0.01
                && (w.range - 275.0).abs() < 0.01
                && (w.min_range - 50.0).abs() < 0.01
                && !w.can_target_air
                && w.can_target_ground
        }
}

/// Wave 63 residual honesty: Fire Base structure body residual peel.
pub fn honesty_fire_base_body_residual_ok() -> bool {
    (FIRE_BASE_MAX_HEALTH - 1_000.0).abs() < 0.01
        && (FIRE_BASE_VISION_RANGE - 360.0).abs() < 0.01
        && (FIRE_BASE_SHROUD_CLEARING_RANGE - 360.0).abs() < 0.01
        && FIRE_BASE_BUILD_COST == 1_000
        && (FIRE_BASE_BUILD_TIME_SEC - 25.0).abs() < 0.01
        && FIRE_BASE_BUILD_TIME_FRAMES
            == ((FIRE_BASE_BUILD_TIME_SEC * FIRE_BASE_LOGIC_FPS).round() as u32)
        && FIRE_BASE_BUILD_TIME_FRAMES == 750
        && FIRE_BASE_ENERGY_PRODUCTION == 0
        && should_apply_fire_base_residual(true)
        && !should_apply_fire_base_residual(false)
}

/// Combined Wave 63 Fire Base residual honesty pack.
pub fn honesty_fire_base_residual_pack_ok() -> bool {
    honesty_fire_base_weapon_residual_ok() && honesty_fire_base_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_base_name_matrix() {
        assert!(is_fire_base_template("AmericaFireBase"));
        assert!(is_fire_base_template("USA_FireBase"));
        assert!(is_fire_base_template("TestFireBase"));
        assert!(is_fire_base_template("AirF_AmericaFireBase"));
        assert!(is_fire_base_template("SupW_AmericaFireBase"));
        assert!(is_fire_base_template("Lazr_AmericaFireBase"));
        assert!(!is_fire_base_template("FireBaseHowitzerGun"));
        assert!(!is_fire_base_template("FireBaseArmor"));
        assert!(!is_fire_base_template("AmericaStrategyCenter"));
        assert!(!is_fire_base_template("AmericaPatriotBattery"));
    }

    #[test]
    fn weapon_and_radius() {
        let w = fire_base_weapon();
        assert!((w.damage - 75.0).abs() < 0.01);
        assert!((w.range - 275.0).abs() < 0.01);
        assert!((w.min_range - 50.0).abs() < 0.01);
        assert!((w.reload_time - 2.0).abs() < 0.05);
        assert!(!w.can_target_air);
        assert!(w.can_target_ground);

        assert!((fire_base_damage_at(0.0) - 75.0).abs() < 0.01);
        assert!((fire_base_damage_at(10.0) - 75.0).abs() < 0.01);
        assert!((fire_base_damage_at(15.0)).abs() < 0.01);
    }

    #[test]
    fn fire_base_residual_pack_honesty_wave63() {
        assert!(honesty_fire_base_weapon_residual_ok());
        assert!(honesty_fire_base_body_residual_ok());
        assert!(honesty_fire_base_residual_pack_ok());
        assert_eq!(fire_base_ms_to_frames(2_000), 60);
        assert_eq!(fire_base_ms_to_frames(0), 0);
        assert_eq!(FIRE_BASE_BUILD_TIME_FRAMES, 750);
        assert!((FIRE_BASE_SCATTER_VS_INFANTRY - 15.0).abs() < 0.01);
        assert_eq!(FIRE_BASE_PROJECTILE, "GenericTankShell");
        assert!(FIRE_BASE_SCALE_WEAPON_SPEED);
    }
}
