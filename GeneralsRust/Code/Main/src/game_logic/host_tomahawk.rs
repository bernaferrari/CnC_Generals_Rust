//! Host America Tomahawk Launcher residual (long-range dual-radius missile).
//!
//! Residual slice (playability):
//! - `AmericaVehicleTomahawk` / `USA_Tomahawk` / variants spawn with PRIMARY
//!   `TomahawkMissileWeapon` (PrimaryDamage **150** / radius **10** +
//!   SecondaryDamage **50** / radius **25**, AttackRange **350**,
//!   MinimumAttackRange **100**, ClipReload **7000**ms → 210 frames).
//! - Fire residual: dual-radius splash (intended + primary/secondary rings).
//! - PreAttackDelay **250**ms residual honesty (recorded; not full PER_SHOT anim lock).
//!
//! Wave 58 residual pack (retail AmericaVehicle.ini / Weapon.ini / WeaponObjects.ini /
//! Locomotor.ini honesty):
//! - Missile loft: FuelLifetime **4000**ms → **120**f, InitialVelocity **50**,
//!   DistanceToTravelBeforeTurning **80**, DistanceToTargetBeforeDiving **100**,
//!   DistanceToTargetForLock **10**, PreferredHeight **120**, PreferredHeightDamping **0.7**
//! - TomahawkMissileLocomotor: Speed **200**, MinSpeed **100**, Acceleration **675**,
//!   TurnRate **540**, MaxThrustAngle **45**
//! - Launcher FirePitch **70**, TurretTurnRate **60**, TurretPitchRate **60**
//! - ScatterRadiusVsInfantry **20**, DelayBetweenShots **1**ms, ClipSize **1**
//! - FireFX FX_TomahawkIgnition, ProjectileObject TomahawkMissile,
//!   ProjectileExhaust TomahawkMissileExhaust, FireSound TomahawkWeapon
//! - Body MaxHealth **180**, VisionRange **180**, ShroudClearingRange **200**,
//!   BuildCost **1200**, TomahawkLocomotor Speed **30**
//!
//! Fail-closed honesty:
//! - Not full TomahawkMissile projectile lob / CapableOfFollowingWaypoints path
//! - Not full PreAttackDelay PER_SHOT anim / hide-show missile bone matrix
//! - Not Scout/Battle/Hellfire drone payload residual (see host_slave_drones)
//! - Not network tomahawk replication (network deferred)

use super::Weapon;

/// Logic frames per second (host fixed step).
pub const TOMAHAWK_LOGIC_FPS: f32 = 30.0;

/// Retail primary weapon.
pub const TOMAHAWK_MISSILE_WEAPON: &str = "TomahawkMissileWeapon";
/// Retail projectile object residual.
pub const TOMAHAWK_MISSILE_PROJECTILE: &str = "TomahawkMissile";
/// Retail projectile locomotor residual.
pub const TOMAHAWK_MISSILE_LOCOMOTOR: &str = "TomahawkMissileLocomotor";
/// Retail vehicle locomotor residual.
pub const TOMAHAWK_VEHICLE_LOCOMOTOR: &str = "TomahawkLocomotor";
/// Residual fire audio.
pub const TOMAHAWK_FIRE_AUDIO: &str = "TomahawkWeapon";
/// Residual FireFX name.
pub const TOMAHAWK_FIRE_FX: &str = "FX_TomahawkIgnition";
/// Residual ProjectileExhaust residual.
pub const TOMAHAWK_PROJECTILE_EXHAUST: &str = "TomahawkMissileExhaust";
/// Residual ProjectileDetonationFX residual (retail BombTruckDefaultBomb).
pub const TOMAHAWK_DETONATION_FX: &str = "WeaponFX_BombTruckDefaultBombDetonation";

/// Retail PrimaryDamage.
pub const TOMAHAWK_PRIMARY_DAMAGE: f32 = 150.0;
/// Retail PrimaryDamageRadius.
pub const TOMAHAWK_PRIMARY_RADIUS: f32 = 10.0;
/// Retail SecondaryDamage.
pub const TOMAHAWK_SECONDARY_DAMAGE: f32 = 50.0;
/// Retail SecondaryDamageRadius.
pub const TOMAHAWK_SECONDARY_RADIUS: f32 = 25.0;
/// Retail AttackRange.
pub const TOMAHAWK_RANGE: f32 = 350.0;
/// Retail MinimumAttackRange.
pub const TOMAHAWK_MIN_RANGE: f32 = 100.0;
/// Retail ScatterRadiusVsInfantry residual.
pub const TOMAHAWK_SCATTER_VS_INFANTRY: f32 = 20.0;
/// Retail DelayBetweenShots residual (msec).
pub const TOMAHAWK_DELAY_BETWEEN_SHOTS_MS: u32 = 1;
/// Retail ClipSize residual.
pub const TOMAHAWK_CLIP_SIZE: u32 = 1;
/// Retail ClipReloadTime residual (msec).
pub const TOMAHAWK_CLIP_RELOAD_MS: u32 = 7_000;
/// Retail ClipReloadTime 7000ms → 210 frames @ 30 FPS.
pub const TOMAHAWK_RELOAD_FRAMES: u32 = 210;
/// Retail PreAttackDelay residual (msec).
pub const TOMAHAWK_PRE_ATTACK_MS: u32 = 250;
/// Retail PreAttackDelay 250ms → 8 frames @ 30 FPS (honesty residual).
pub const TOMAHAWK_PRE_ATTACK_FRAMES: u32 = 8;
/// Residual projectile speed (TomahawkMissileLocomotor Speed = 200).
pub const TOMAHAWK_PROJECTILE_SPEED: f32 = 200.0;
/// Retail CapableOfFollowingWaypoints residual.
pub const TOMAHAWK_CAPABLE_OF_FOLLOWING_WAYPOINTS: bool = true;

// --- Missile loft residual (MissileAIUpdate + TomahawkMissileLocomotor) ---

/// Retail FuelLifetime residual (msec).
pub const TOMAHAWK_FUEL_LIFETIME_MS: u32 = 4_000;
/// FuelLifetime 4000ms → 120 frames @ 30 FPS.
pub const TOMAHAWK_FUEL_LIFETIME_FRAMES: u32 = 120;
/// Retail InitialVelocity residual (dist/sec).
pub const TOMAHAWK_INITIAL_VELOCITY: f32 = 50.0;
/// Retail DistanceToTravelBeforeTurning residual.
pub const TOMAHAWK_DISTANCE_BEFORE_TURNING: f32 = 80.0;
/// Retail DistanceToTargetBeforeDiving residual.
pub const TOMAHAWK_DISTANCE_BEFORE_DIVING: f32 = 100.0;
/// Retail DistanceToTargetForLock residual.
pub const TOMAHAWK_DISTANCE_FOR_LOCK: f32 = 10.0;
/// Retail IgnitionDelay residual (msec).
pub const TOMAHAWK_IGNITION_DELAY_MS: u32 = 0;
/// Retail PreferredHeight residual (missile loft).
pub const TOMAHAWK_PREFERRED_HEIGHT: f32 = 120.0;
/// Retail PreferredHeightDamping residual.
pub const TOMAHAWK_PREFERRED_HEIGHT_DAMPING: f32 = 0.7;
/// Retail MinSpeed residual (locomotor).
pub const TOMAHAWK_MISSILE_MIN_SPEED: f32 = 100.0;
/// Retail Acceleration residual.
pub const TOMAHAWK_MISSILE_ACCELERATION: f32 = 675.0;
/// Retail TurnRate residual (degrees/sec).
pub const TOMAHAWK_MISSILE_TURN_RATE: f32 = 540.0;
/// Retail MaxThrustAngle residual (degrees).
pub const TOMAHAWK_MAX_THRUST_ANGLE: f32 = 45.0;
/// Retail AirborneTargetingHeight residual.
pub const TOMAHAWK_AIRBORNE_TARGETING_HEIGHT: f32 = 30.0;
/// Retail TryToFollowTarget residual.
pub const TOMAHAWK_TRY_TO_FOLLOW_TARGET: bool = true;

// --- Launcher vehicle residual ---

/// Retail FirePitch residual (degrees) — aim pitch instead of target pitch.
pub const TOMAHAWK_FIRE_PITCH: f32 = 70.0;
/// Retail TurretTurnRate residual (degrees/sec).
pub const TOMAHAWK_TURRET_TURN_RATE: f32 = 60.0;
/// Retail TurretPitchRate residual (degrees/sec).
pub const TOMAHAWK_TURRET_PITCH_RATE: f32 = 60.0;
/// Retail MaxHealth residual.
pub const TOMAHAWK_MAX_HEALTH: f32 = 180.0;
/// Retail VisionRange residual.
pub const TOMAHAWK_VISION_RANGE: f32 = 180.0;
/// Retail ShroudClearingRange residual.
pub const TOMAHAWK_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail BuildCost residual.
pub const TOMAHAWK_BUILD_COST: u32 = 1_200;
/// Retail BuildTime residual (seconds).
pub const TOMAHAWK_BUILD_TIME_SEC: f32 = 20.0;
/// Retail TomahawkLocomotor Speed residual.
pub const TOMAHAWK_VEHICLE_SPEED: f32 = 30.0;
/// Retail TransportSlotCount residual.
pub const TOMAHAWK_TRANSPORT_SLOT_COUNT: u32 = 3;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn tomahawk_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / TOMAHAWK_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual Tomahawk launcher vehicle.
///
/// Fail-closed: name residual. Excludes missiles/projectiles/hulks.
pub fn is_tomahawk_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "usa_tomahawk"
        || n == "usa_tomahawklauncher"
        || n == "testtomahawk"
        || n == "americavehicletomahawk"
    {
        return true;
    }
    // Exclude missiles, weapons, hulks, locomotor, exhaust, crates.
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("exhaust")
        || n.contains("detonation")
        || n.ends_with("missile")
        || n.contains("missileweapon")
    {
        return false;
    }
    // Living vehicle residual: *VehicleTomahawk* / *Tomahawk* chassis names.
    n.contains("vehicletomahawk") || (n.contains("tomahawk") && !n.contains("missile"))
}

/// Whether residual fire should apply Tomahawk residual path.
pub fn should_apply_tomahawk_residual(is_tomahawk: bool) -> bool {
    is_tomahawk
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Tomahawk primary Weapon.
pub fn tomahawk_weapon() -> Weapon {
    Weapon {
        damage: TOMAHAWK_PRIMARY_DAMAGE,
        range: TOMAHAWK_RANGE,
        min_range: TOMAHAWK_MIN_RANGE,
        reload_time: delay_frames_to_reload_secs(TOMAHAWK_RELOAD_FRAMES),
        last_fire_time: 0.0,
        ammo: Some(TOMAHAWK_CLIP_SIZE),
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: TOMAHAWK_PROJECTILE_SPEED,
        pre_attack_delay: delay_frames_to_reload_secs(TOMAHAWK_PRE_ATTACK_FRAMES),
        splash_radius: 0.0,
    }
}

/// Dual-radius residual damage at distance from impact (max of rings).
///
/// Intended target at impact takes PrimaryDamage; nearby units within
/// PrimaryDamageRadius take PrimaryDamage; SecondaryDamageRadius takes
/// SecondaryDamage residual.
pub fn tomahawk_damage_at(distance_from_impact: f32) -> f32 {
    if distance_from_impact <= TOMAHAWK_PRIMARY_RADIUS {
        TOMAHAWK_PRIMARY_DAMAGE
    } else if distance_from_impact <= TOMAHAWK_SECONDARY_RADIUS {
        TOMAHAWK_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual splash target.
///
/// Retail RadiusDamageAffects includes SELF + ALLIES (friendly fire residual).
/// Host residual still skips self-source and under-construction; allies are
/// damageable (artillery friendly-fire residual honesty).
pub fn is_legal_tomahawk_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Residual composite loft frames: fuel lifetime honesty (fail-closed; not full
/// DistanceToTravelBeforeTurning / dive physics).
pub fn tomahawk_loft_composite_frames() -> u32 {
    TOMAHAWK_FUEL_LIFETIME_FRAMES
}

/// Residual: whether impact range is within AttackRange and outside MinRange.
pub fn tomahawk_range_legal(distance: f32) -> bool {
    distance >= TOMAHAWK_MIN_RANGE && distance <= TOMAHAWK_RANGE
}

// --- Wave 58 residual honesty packs ---

/// Wave 58 residual honesty: dual-radius weapon residual.
pub fn honesty_tomahawk_weapon_residual_ok() -> bool {
    TOMAHAWK_MISSILE_WEAPON == "TomahawkMissileWeapon"
        && (TOMAHAWK_PRIMARY_DAMAGE - 150.0).abs() < 0.01
        && (TOMAHAWK_PRIMARY_RADIUS - 10.0).abs() < 0.01
        && (TOMAHAWK_SECONDARY_DAMAGE - 50.0).abs() < 0.01
        && (TOMAHAWK_SECONDARY_RADIUS - 25.0).abs() < 0.01
        && (TOMAHAWK_RANGE - 350.0).abs() < 0.01
        && (TOMAHAWK_MIN_RANGE - 100.0).abs() < 0.01
        && (TOMAHAWK_SCATTER_VS_INFANTRY - 20.0).abs() < 0.01
        && TOMAHAWK_CLIP_RELOAD_MS == 7_000
        && TOMAHAWK_RELOAD_FRAMES == tomahawk_ms_to_frames(TOMAHAWK_CLIP_RELOAD_MS)
        && TOMAHAWK_PRE_ATTACK_MS == 250
        && TOMAHAWK_PRE_ATTACK_FRAMES == tomahawk_ms_to_frames(TOMAHAWK_PRE_ATTACK_MS)
        && TOMAHAWK_CLIP_SIZE == 1
        && TOMAHAWK_DELAY_BETWEEN_SHOTS_MS == 1
        && TOMAHAWK_CAPABLE_OF_FOLLOWING_WAYPOINTS
        && TOMAHAWK_FIRE_AUDIO == "TomahawkWeapon"
        && TOMAHAWK_FIRE_FX == "FX_TomahawkIgnition"
        && TOMAHAWK_DETONATION_FX == "WeaponFX_BombTruckDefaultBombDetonation"
        && (tomahawk_damage_at(0.0) - 150.0).abs() < 0.01
        && (tomahawk_damage_at(15.0) - 50.0).abs() < 0.01
        && tomahawk_range_legal(200.0)
        && !tomahawk_range_legal(50.0)
        && !tomahawk_range_legal(400.0)
}

/// Wave 58 residual honesty: missile loft residual.
pub fn honesty_tomahawk_loft_residual_ok() -> bool {
    TOMAHAWK_MISSILE_PROJECTILE == "TomahawkMissile"
        && TOMAHAWK_MISSILE_LOCOMOTOR == "TomahawkMissileLocomotor"
        && TOMAHAWK_FUEL_LIFETIME_MS == 4_000
        && TOMAHAWK_FUEL_LIFETIME_FRAMES == tomahawk_ms_to_frames(TOMAHAWK_FUEL_LIFETIME_MS)
        && (TOMAHAWK_INITIAL_VELOCITY - 50.0).abs() < 0.01
        && (TOMAHAWK_DISTANCE_BEFORE_TURNING - 80.0).abs() < 0.01
        && (TOMAHAWK_DISTANCE_BEFORE_DIVING - 100.0).abs() < 0.01
        && (TOMAHAWK_DISTANCE_FOR_LOCK - 10.0).abs() < 0.01
        && TOMAHAWK_IGNITION_DELAY_MS == 0
        && (TOMAHAWK_PREFERRED_HEIGHT - 120.0).abs() < 0.01
        && (TOMAHAWK_PREFERRED_HEIGHT_DAMPING - 0.7).abs() < 0.01
        && (TOMAHAWK_PROJECTILE_SPEED - 200.0).abs() < 0.01
        && (TOMAHAWK_MISSILE_MIN_SPEED - 100.0).abs() < 0.01
        && (TOMAHAWK_MISSILE_ACCELERATION - 675.0).abs() < 0.01
        && (TOMAHAWK_MISSILE_TURN_RATE - 540.0).abs() < 0.01
        && (TOMAHAWK_MAX_THRUST_ANGLE - 45.0).abs() < 0.01
        && (TOMAHAWK_AIRBORNE_TARGETING_HEIGHT - 30.0).abs() < 0.01
        && TOMAHAWK_TRY_TO_FOLLOW_TARGET
        && tomahawk_loft_composite_frames() == 120
}

/// Wave 58 residual honesty: launcher vehicle residual.
pub fn honesty_tomahawk_launcher_residual_ok() -> bool {
    (TOMAHAWK_FIRE_PITCH - 70.0).abs() < 0.01
        && (TOMAHAWK_TURRET_TURN_RATE - 60.0).abs() < 0.01
        && (TOMAHAWK_TURRET_PITCH_RATE - 60.0).abs() < 0.01
        && (TOMAHAWK_MAX_HEALTH - 180.0).abs() < 0.01
        && (TOMAHAWK_VISION_RANGE - 180.0).abs() < 0.01
        && (TOMAHAWK_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && TOMAHAWK_BUILD_COST == 1_200
        && (TOMAHAWK_BUILD_TIME_SEC - 20.0).abs() < 0.01
        && (TOMAHAWK_VEHICLE_SPEED - 30.0).abs() < 0.01
        && TOMAHAWK_TRANSPORT_SLOT_COUNT == 3
        && TOMAHAWK_VEHICLE_LOCOMOTOR == "TomahawkLocomotor"
        && TOMAHAWK_PROJECTILE_EXHAUST == "TomahawkMissileExhaust"
}

/// Combined Wave 58 Tomahawk residual honesty pack.
pub fn honesty_tomahawk_residual_pack_ok() -> bool {
    honesty_tomahawk_weapon_residual_ok()
        && honesty_tomahawk_loft_residual_ok()
        && honesty_tomahawk_launcher_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tomahawk_name_matrix() {
        assert!(is_tomahawk_template("AmericaVehicleTomahawk"));
        assert!(is_tomahawk_template("USA_Tomahawk"));
        assert!(is_tomahawk_template("USA_TomahawkLauncher"));
        assert!(is_tomahawk_template("TestTomahawk"));
        assert!(is_tomahawk_template("SupW_AmericaVehicleTomahawk"));
        assert!(!is_tomahawk_template("TomahawkMissile"));
        assert!(!is_tomahawk_template("TomahawkMissileWeapon"));
        assert!(!is_tomahawk_template("TomahawkMissileLocomotor"));
        assert!(!is_tomahawk_template("AmericaVehicleTomahawkHulk"));
        assert!(!is_tomahawk_template("USA_Crusader"));
        assert!(!is_tomahawk_template("USA_Ranger"));
    }

    #[test]
    fn weapon_and_dual_radius() {
        let w = tomahawk_weapon();
        assert!((w.damage - 150.0).abs() < 0.01);
        assert!((w.range - 350.0).abs() < 0.01);
        assert!((w.min_range - 100.0).abs() < 0.01);
        assert!((w.reload_time - 7.0).abs() < 0.05);

        assert!((tomahawk_damage_at(0.0) - 150.0).abs() < 0.01);
        assert!((tomahawk_damage_at(10.0) - 150.0).abs() < 0.01);
        assert!((tomahawk_damage_at(15.0) - 50.0).abs() < 0.01);
        assert!((tomahawk_damage_at(30.0)).abs() < 0.01);
    }

    #[test]
    fn tomahawk_residual_pack_honesty_wave58() {
        assert!(honesty_tomahawk_residual_pack_ok());
        assert_eq!(tomahawk_ms_to_frames(7_000), 210);
        assert_eq!(tomahawk_ms_to_frames(250), 8);
        assert_eq!(tomahawk_ms_to_frames(4_000), 120);
        assert_eq!(tomahawk_loft_composite_frames(), 120);
        assert!((TOMAHAWK_PREFERRED_HEIGHT - 120.0).abs() < 0.01);
        assert!((TOMAHAWK_DISTANCE_BEFORE_TURNING - 80.0).abs() < 0.01);
        assert!((TOMAHAWK_DISTANCE_BEFORE_DIVING - 100.0).abs() < 0.01);
        assert!(tomahawk_range_legal(350.0));
        assert!(!tomahawk_range_legal(99.0));
    }
}
