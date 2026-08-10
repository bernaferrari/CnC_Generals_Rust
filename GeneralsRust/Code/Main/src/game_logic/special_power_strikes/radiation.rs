//! Nuke radiation / NuclearMissile residual constants.
use super::types::*;
// --- Nuke radiation residual (retail NukeRadiationFieldWeapon / LifetimeUpdate) ---

/// Retail `NukeRadiationFieldWeapon` PrimaryDamage.
pub const NUKE_RADIATION_DAMAGE_PER_TICK: f32 = 25.0;
/// Retail `NukeRadiationFieldWeapon` PrimaryDamageRadius.
pub const NUKE_RADIATION_RADIUS: f32 = 200.0;
/// Retail DelayBetweenShots = 750 ms → ~23 frames @ 30 FPS.
pub const NUKE_RADIATION_TICK_INTERVAL_FRAMES: u32 = 23;
/// Retail NukeRadiationFieldWeapon LifetimeUpdate Min/MaxLifetime = 30000 ms @ 30 FPS.
pub const NUKE_RADIATION_DURATION_FRAMES: u32 = 900;
/// Residual ambient cue for the radiation pool.
pub const NUKE_RADIATION_AUDIO: &str = "RadiationPoolAmbientLoop";
/// Retail `NukeRadiationFieldWeapon` FireFX residual.
pub const NUKE_RADIATION_FIRE_FX: &str = "WeaponFX_LargeRadiationFieldWeapon";
/// Retail `NukeRadiationFieldWeapon` DamageType residual.
pub const NUKE_RADIATION_DAMAGE_TYPE: &str = "RADIATION";
/// Retail `NukeRadiationFieldWeapon` DeathType residual.
pub const NUKE_RADIATION_DEATH_TYPE: &str = "NORMAL";
/// Retail `NukeRadiationFieldWeapon` WeaponSpeed residual (dist/sec).
pub const NUKE_RADIATION_WEAPON_SPEED: f32 = 600.0;
/// Retail `NukeRadiationFieldWeapon` SuspendFXDelay = 10000 ms → 300 frames @ 30 FPS.
pub const NUKE_RADIATION_SUSPEND_FX_DELAY_MS: u32 = 10000;
/// SuspendFXDelay frames residual (ceil 10000*30/1000).
pub const NUKE_RADIATION_SUSPEND_FX_DELAY_FRAMES: u32 = 300;
/// Retail OCL residual for nuke radiation field spawn.
pub const NUKE_RADIATION_OCL: &str = "OCL_NukeRadiationField";
/// Retail object name spawned by OCL_NukeRadiationField.
pub const NUKE_RADIATION_OBJECT_NAME: &str = "NukeRadiationFieldWeapon";
/// Retail NukeRadiationFieldWeapon weapon template residual.
pub const NUKE_RADIATION_WEAPON_NAME: &str = "NukeRadiationFieldWeapon";
/// Retail NukeRadiationFieldWeapon LifetimeUpdate Min/MaxLifetime msec.
pub const NUKE_RADIATION_LIFETIME_MS: u32 = 30000;
/// Retail NukeRadiationFieldWeapon DelayBetweenShots msec residual.
pub const NUKE_RADIATION_DELAY_BETWEEN_SHOTS_MS: u32 = 750;
/// Retail NukeRadiationFieldWeapon RadiusDamageAffects residual.
pub const NUKE_RADIATION_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS NOT_AIRBORNE";
/// Retail NukeRadiationFieldWeapon MaxHealth residual (field object body).
pub const NUKE_RADIATION_FIELD_MAX_HEALTH: f32 = 150.0;
/// Retail NukeRadiationFieldWeapon GeometryMajorRadius residual.
pub const NUKE_RADIATION_GEOMETRY_MAJOR_RADIUS: f32 = 100.0;

// --- Wave 73: NuclearMissile radiation residual pack deepen ---

/// Retail `NukeRadiationFieldWeapon` AttackRange residual.
pub const NUKE_RADIATION_ATTACK_RANGE: f32 = 15.0;
/// Retail `NukeRadiationFieldWeapon` MinimumAttackRange residual.
pub const NUKE_RADIATION_MINIMUM_ATTACK_RANGE: f32 = 10.0;
/// Retail NukeRadiationFieldWeapon KindOf residual.
pub const NUKE_RADIATION_KIND_OF: &str = "IMMOBILE CLEANUP_HAZARD INERT NO_COLLIDE";
/// Retail NukeRadiationFieldWeapon Armor residual.
pub const NUKE_RADIATION_ARMOR: &str = "HazardousMaterialArmor";
/// Retail NukeRadiationFieldWeapon Geometry residual.
pub const NUKE_RADIATION_GEOMETRY: &str = "CYLINDER";
/// Retail NukeRadiationFieldWeapon GeometryHeight residual.
pub const NUKE_RADIATION_GEOMETRY_HEIGHT: f32 = 1.0;
/// Retail NukeRadiationFieldWeapon GeometryIsSmall residual.
pub const NUKE_RADIATION_GEOMETRY_IS_SMALL: bool = false;
/// Retail NukeRadiationFieldWeapon InitialHealth residual.
pub const NUKE_RADIATION_FIELD_INITIAL_HEALTH: f32 = 150.0;
/// Retail NukeRadiationFieldWeapon EditorSorting residual.
pub const NUKE_RADIATION_EDITOR_SORTING: &str = "SYSTEM";
/// Retail NukeRadiationFieldWeapon HazardFieldCoreWeapon residual (stack clean).
pub const NUKE_RADIATION_HAZARD_FIELD_CORE_WEAPON: &str = "HazardFieldCoreWeapon";
/// Retail NukeRadiationFieldWeapon FXListDie DeathFX residual.
pub const NUKE_RADIATION_DEATH_FX: &str = "FX_RadiationPoolDie";
/// Retail SuperweaponNeutronMissile ReloadTime residual (msec).
pub const NUCLEAR_MISSILE_RELOAD_MS: u32 = 360_000;
/// SuperweaponNeutronMissile ReloadTime 360000ms → 10800 frames @ 30 FPS.
pub const NUCLEAR_MISSILE_RELOAD_FRAMES: u32 = 10_800;
/// Retail SuperweaponNeutronMissile RadiusCursorRadius residual.
pub const NUCLEAR_MISSILE_RADIUS_CURSOR: f32 = 210.0;
/// Retail SuperweaponNeutronMissile ViewObjectDuration residual (msec).
pub const NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_MS: u32 = 40_000;
/// ViewObjectDuration 40000ms → 1200 frames.
pub const NUCLEAR_MISSILE_VIEW_OBJECT_DURATION_FRAMES: u32 = 1_200;
/// Retail SuperweaponNeutronMissile ViewObjectRange residual.
pub const NUCLEAR_MISSILE_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SuperweaponNeutronMissile InitiateAtLocationSound residual.
pub const NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND: &str = "AirRaidSiren";
/// Wave 77: SuperweaponNeutronMissile InitiateSound is commented out in retail
/// SpecialPower.ini — residual empty (fail-closed vs inventing AirRaidSiren at source).
pub const NUCLEAR_MISSILE_INITIATE_SOUND: &str = "";

// --- Wave 73: SupW / Nuke_ / AirF special-power variant residual pack ---

/// Retail SupW_SuperweaponNeutronMissile ReloadTime residual (msec).
pub const SUPW_NEUTRON_MISSILE_RELOAD_MS: u32 = 240_000;
/// SupW Neutron ReloadTime 240000ms → 7200 frames.
pub const SUPW_NEUTRON_MISSILE_RELOAD_FRAMES: u32 = 7_200;
/// Retail SupW_SuperweaponNeutronMissile RadiusCursorRadius residual.
pub const SUPW_NEUTRON_MISSILE_RADIUS_CURSOR: f32 = 210.0;
/// Retail SupW_SuperweaponNeutronMissile ViewObjectDuration residual (msec).
pub const SUPW_NEUTRON_MISSILE_VIEW_OBJECT_DURATION_MS: u32 = 40_000;
/// ViewObjectDuration 40000ms → 1200 frames.
pub const SUPW_NEUTRON_MISSILE_VIEW_OBJECT_DURATION_FRAMES: u32 = 1_200;
/// Retail SupW_SuperweaponNeutronMissile ViewObjectRange residual.
pub const SUPW_NEUTRON_MISSILE_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SupW_SuperweaponNeutronMissile template residual name.
pub const SUPW_NEUTRON_MISSILE_SPECIAL_POWER: &str = "SupW_SuperweaponNeutronMissile";
/// Retail SupW_SuperweaponParticleUplinkCannon ReloadTime residual (msec).
pub const SUPW_PUC_RELOAD_MS: u32 = 180_000;
/// SupW PUC ReloadTime 180000ms → 5400 frames.
pub const SUPW_PUC_RELOAD_FRAMES: u32 = 5_400;
/// Retail SupW_SuperweaponParticleUplinkCannon template residual name.
pub const SUPW_PUC_SPECIAL_POWER: &str = "SupW_SuperweaponParticleUplinkCannon";
/// Retail Nuke_SuperweaponNeutronMissile ReloadTime residual (msec).
pub const NUKE_GENERAL_NEUTRON_RELOAD_MS: u32 = 300_000;
/// Nuke_ Neutron ReloadTime 300000ms → 9000 frames.
pub const NUKE_GENERAL_NEUTRON_RELOAD_FRAMES: u32 = 9_000;
/// Retail Nuke_SuperweaponNeutronMissile RadiusCursorRadius residual.
pub const NUKE_GENERAL_NEUTRON_RADIUS_CURSOR: f32 = 210.0;
/// Retail Nuke_SuperweaponNeutronMissile template residual name.
pub const NUKE_GENERAL_NEUTRON_SPECIAL_POWER: &str = "Nuke_SuperweaponNeutronMissile";
