//! CruiseMissile / MOAB / DaisyCutter residual constants.
use super::types::*;
// --- Cruise Missile residual (retail SupW_CruiseMissile / MOABDetonationWeapon) ---

/// Retail `MOABDetonationWeapon` PrimaryDamage (CruiseMissile FireWeaponWhenDead).
pub const CRUISE_MISSILE_DAMAGE: f32 = 2000.0;
/// Retail `MOABDetonationWeapon` PrimaryDamageRadius.
pub const CRUISE_MISSILE_RADIUS: f32 = 150.0;
/// Residual inner full-damage radius (host falloff; retail MOAB is flat primary).
pub const CRUISE_MISSILE_FALLOFF_INNER: f32 = 90.0;
/// Residual loft/approach frames before impact damage applies
/// (fail-closed vs full NeutronMissileUpdate DistanceToTravelBeforeTurning /
/// SpecialSpeedTime / HeightDieUpdate / MissileLauncherBuildingUpdate doors).
pub const CRUISE_MISSILE_IMPACT_DELAY_FRAMES: u32 = 180;

// --- MOABFlameWeapon secondary residual (MOABGas SlowDeath MIDPOINT / tree-ignite) ---

/// Retail `MOABFlameWeapon` PrimaryDamage (spot of flame to light trees).
pub const MOAB_FLAME_DAMAGE: f32 = 5.0;
/// Retail `MOABFlameWeapon` PrimaryDamageRadius.
pub const MOAB_FLAME_RADIUS: f32 = 100.0;
/// Residual honesty audio / FX label for flame secondary.
pub const MOAB_FLAME_AUDIO: &str = "FX_MOABIgnite";
/// Retail CruiseMissileWeapon ProjectileObject residual.
pub const CRUISE_MISSILE_PROJECTILE_OBJECT: &str = "CruiseMissile";
/// Retail SUPERWEAPON_CruiseMissile FireWeapon residual name.
pub const CRUISE_MISSILE_WEAPON_NAME: &str = "CruiseMissileWeapon";
/// Retail OCL residual for SupW cruise launch.
pub const CRUISE_MISSILE_OCL: &str = "SUPERWEAPON_CruiseMissile";
/// Retail FireWeaponWhenDead DeathWeapon residual.
pub const CRUISE_MISSILE_DEATH_WEAPON: &str = "MOABDetonationWeapon";
/// Retail MOABDetonationWeapon FireFX residual.
pub const CRUISE_MISSILE_MOAB_FIRE_FX: &str = "WeaponFX_MOAB_Blast";
/// Retail CruiseMissileWeapon FireFX residual.
pub const CRUISE_MISSILE_LAUNCH_FIRE_FX: &str = "WeaponFX_NeutronMissile";
/// Retail NeutronMissileUpdate LaunchFX residual.
pub const CRUISE_MISSILE_LAUNCH_FX: &str = "FX_NeutronMissileLaunch";
/// Retail NeutronMissileUpdate IgnitionFX residual.
pub const CRUISE_MISSILE_IGNITION_FX: &str = "FX_NeutronMissileIgnition";
/// Retail CruiseMissileWeapon ProjectileExhaust residual.
pub const CRUISE_MISSILE_EXHAUST: &str = "NeutronMissileExhaust";
/// Retail NeutronMissileUpdate DistanceToTravelBeforeTurning residual.
pub const CRUISE_MISSILE_DISTANCE_BEFORE_TURNING: f32 = 200.0;
/// Retail NeutronMissileUpdate SpecialSpeedTime = 1500 ms residual.
pub const CRUISE_MISSILE_SPECIAL_SPEED_TIME_MS: u32 = 1500;
/// SpecialSpeedTime frames residual (ceil 1500*30/1000 = 45).
pub const CRUISE_MISSILE_SPECIAL_SPEED_TIME_FRAMES: u32 = 45;
/// Retail NeutronMissileUpdate SpecialSpeedHeight residual.
pub const CRUISE_MISSILE_SPECIAL_SPEED_HEIGHT: f32 = 160.0;
/// Retail NeutronMissileUpdate SpecialJitterDistance residual.
pub const CRUISE_MISSILE_SPECIAL_JITTER_DISTANCE: f32 = 0.4;
/// Retail NeutronMissileUpdate TargetFromDirectlyAbove residual.
pub const CRUISE_MISSILE_TARGET_FROM_ABOVE: f32 = 10.0;
/// Retail HeightDieUpdate TargetHeight residual.
pub const CRUISE_MISSILE_HEIGHT_DIE_TARGET: f32 = 10.0;
/// Retail HeightDieUpdate InitialDelay = 1000 ms residual.
pub const CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_MS: u32 = 1000;
/// HeightDie InitialDelay frames residual (1000 ms → 30).
pub const CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES: u32 = 30;
/// Retail DeliveryDecalRadius residual on CruiseMissile.
pub const CRUISE_MISSILE_DECAL_RADIUS: f32 = 210.0;
/// Retail MissileLauncherBuildingUpdate DoorOpenTime residual (msec).
pub const CRUISE_MISSILE_DOOR_OPEN_TIME_MS: u32 = 8000;
/// DoorOpenTime frames residual (8000 ms → 240).
pub const CRUISE_MISSILE_DOOR_OPEN_TIME_FRAMES: u32 = 240;
/// Retail DoorWaitOpenTime residual (msec).
pub const CRUISE_MISSILE_DOOR_WAIT_OPEN_TIME_MS: u32 = 2000;
/// DoorWaitOpenTime frames residual (2000 ms → 60).
pub const CRUISE_MISSILE_DOOR_WAIT_OPEN_TIME_FRAMES: u32 = 60;
/// Retail SupW_CruiseMissile ReloadTime residual (msec).
pub const CRUISE_MISSILE_RELOAD_MS: u32 = 120000;
/// ReloadTime frames residual (120000 ms → 3600).
pub const CRUISE_MISSILE_RELOAD_FRAMES: u32 = 3600;
/// Retail SupW_CruiseMissile RadiusCursorRadius residual.
pub const CRUISE_MISSILE_RADIUS_CURSOR: f32 = 210.0;
/// Retail SupW_CruiseMissile InitiateSound residual.
pub const CRUISE_MISSILE_INITIATE_SOUND: &str = "AirRaidSiren";
/// Retail SupW_CruiseMissile InitiateAtLocationSound residual (Wave 77 audio name table).
pub const CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND: &str = "AirRaidSiren";
/// Retail CruiseMissile GeometryMajorRadius residual.
pub const CRUISE_MISSILE_GEOMETRY_MAJOR_RADIUS: f32 = 7.0;
/// Retail CruiseMissile GeometryHeight residual.
pub const CRUISE_MISSILE_GEOMETRY_HEIGHT: f32 = 60.0;
/// Retail MOABDetonationWeapon ShockWaveAmount residual.
pub const MOAB_SHOCKWAVE_AMOUNT: f32 = 250.0;
/// Retail MOABDetonationWeapon ShockWaveRadius residual.
pub const MOAB_SHOCKWAVE_RADIUS: f32 = 200.0;
/// Retail MOABDetonationWeapon ShockWaveTaperOff residual.
pub const MOAB_SHOCKWAVE_TAPER_OFF: f32 = 0.33;
/// Retail MOABDetonationWeapon DamageType residual.
pub const MOAB_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail MOABDetonationWeapon DeathType residual.
pub const MOAB_DEATH_TYPE: &str = "EXPLODED";
/// Retail MOABFlameWeapon DamageType residual.
pub const MOAB_FLAME_DAMAGE_TYPE: &str = "FLAME";
/// Retail MOABFlameWeapon DeathType residual.
pub const MOAB_FLAME_DEATH_TYPE: &str = "BURNED";
/// Host residual loft composition: SpecialSpeedTime + HeightDie InitialDelay
/// (door times deferred; impact delay stays CRUISE_MISSILE_IMPACT_DELAY_FRAMES).
pub const CRUISE_MISSILE_LOFT_COMPOSITE_FRAMES: u32 =
    CRUISE_MISSILE_SPECIAL_SPEED_TIME_FRAMES + CRUISE_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES;

// --- DaisyCutter special-power residual pack (Wave 72, SpecialPower.ini + Weapon.ini) ---

/// Retail SuperweaponDaisyCutter / SuperweaponMOAB ReloadTime residual (msec).
pub const DAISY_CUTTER_RELOAD_MS: u32 = 360_000;
/// ReloadTime frames residual (360000 ms → 10800 @ 30 FPS).
pub const DAISY_CUTTER_RELOAD_FRAMES: u32 = 10_800;
/// Retail SuperweaponDaisyCutter RadiusCursorRadius residual (shared by MOAB).
pub const DAISY_CUTTER_RADIUS_CURSOR: f32 = 170.0;
/// Retail SuperweaponDaisyCutter RequiredScience residual.
pub const DAISY_CUTTER_REQUIRED_SCIENCE: &str = "SCIENCE_DaisyCutter";
/// Retail SuperweaponDaisyCutter template residual name.
pub const DAISY_CUTTER_SPECIAL_POWER: &str = "SuperweaponDaisyCutter";
/// Retail SuperweaponMOAB upgrade residual name (same SPECIAL_DAISY_CUTTER enum).
pub const DAISY_CUTTER_MOAB_SPECIAL_POWER: &str = "SuperweaponMOAB";
/// Retail ViewObjectDuration residual (msec).
pub const DAISY_CUTTER_VIEW_OBJECT_DURATION_MS: u32 = 30_000;
/// ViewObjectDuration frames residual (30000 ms → 900).
pub const DAISY_CUTTER_VIEW_OBJECT_DURATION_FRAMES: u32 = 900;
/// Retail ViewObjectRange residual.
pub const DAISY_CUTTER_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SharedSyncedTimer residual.
pub const DAISY_CUTTER_SHARED_SYNCED_TIMER: bool = true;
/// Retail PublicTimer residual.
pub const DAISY_CUTTER_PUBLIC_TIMER: bool = false;
/// Retail ShortcutPower residual.
pub const DAISY_CUTTER_SHORTCUT_POWER: bool = true;
/// Retail DaisyCutterDetonationWeapon PrimaryDamage residual.
pub const DAISY_CUTTER_PRIMARY_DAMAGE: f32 = 2000.0;
/// Retail DaisyCutterDetonationWeapon PrimaryDamageRadius residual.
pub const DAISY_CUTTER_PRIMARY_RADIUS: f32 = 100.0;
/// Host residual outer damage radius (RadiusCursorRadius residual for falloff).
pub const DAISY_CUTTER_OUTER_RADIUS: f32 = 170.0;
/// FuelAirBombPower residual impact delay frames (3.0s @ 30 FPS).
pub const DAISY_CUTTER_IMPACT_DELAY_FRAMES: u32 = 90;
/// Retail DaisyCutterDetonationWeapon DamageType residual.
pub const DAISY_CUTTER_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DaisyCutterDetonationWeapon DeathType residual.
pub const DAISY_CUTTER_DEATH_TYPE: &str = "EXPLODED";
/// Retail DaisyCutterFlameWeapon PrimaryDamage residual (tree-ignite secondary).
pub const DAISY_CUTTER_FLAME_DAMAGE: f32 = 5.0;
/// Retail DaisyCutterFlameWeapon PrimaryDamageRadius residual.
pub const DAISY_CUTTER_FLAME_RADIUS: f32 = 100.0;
/// Host residual impact audio cue.
pub const DAISY_CUTTER_EXPLOSION_AUDIO: &str = "DaisyCutterExplosion";
