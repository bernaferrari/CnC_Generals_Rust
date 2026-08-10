//! ScudStorm loft, anthrax-tier, and multi-missile residual helpers.
use super::types::*;
/// Scud thrust wobble residual at frame index (sinusoidal host residual).
///
/// C++ Locomotor thrust wobble oscillates between MinWobble and MaxWobble at
/// ThrustWobbleRate. Host residual samples a deterministic sine for honesty.
#[inline]
pub fn scud_missile_thrust_wobble(frame: u32) -> f32 {
    let mid = (SCUD_STORM_MISSILE_THRUST_MIN_WOBBLE + SCUD_STORM_MISSILE_THRUST_MAX_WOBBLE) * 0.5;
    let amp = (SCUD_STORM_MISSILE_THRUST_MAX_WOBBLE - SCUD_STORM_MISSILE_THRUST_MIN_WOBBLE) * 0.5;
    let phase = frame as f32 * SCUD_STORM_MISSILE_THRUST_WOBBLE_RATE;
    mid + amp * phase.sin()
}

// --- ScudStorm multi-missile residual (retail ScudStormWeapon / ScudStormDamageWeapon) ---

/// Retail `ScudStormWeapon` ClipSize (missiles per storm).
pub const SCUD_STORM_MISSILE_COUNT: u32 = 9;
/// Retail `ScatterTargetScalar` (scales ScatterTarget table entries).
pub const SCUD_STORM_SCATTER_SCALAR: f32 = 120.0;
/// Retail `ScudStormDamageWeapon` PrimaryDamage (per missile epicenter).
pub const SCUD_STORM_PRIMARY_DAMAGE: f32 = 500.0;
/// Retail `ScudStormDamageWeapon` PrimaryDamageRadius.
pub const SCUD_STORM_PRIMARY_RADIUS: f32 = 50.0;
/// Retail `ScudStormDamageWeapon` SecondaryDamage.
pub const SCUD_STORM_SECONDARY_DAMAGE: f32 = 150.0;
/// Retail `ScudStormDamageWeaponUpgraded` SecondaryDamage (`Upgrade_GLAAnthraxBeta`).
pub const SCUD_STORM_SECONDARY_DAMAGE_UPGRADED: f32 = 200.0;
/// Retail `ScudStormDamageWeapon` SecondaryDamageRadius.
pub const SCUD_STORM_SECONDARY_RADIUS: f32 = 200.0;
/// Retail PreAttackDelay = 3000 ms → 90 frames @ 30 FPS (first missile due).
pub const SCUD_STORM_PRE_ATTACK_FRAMES: u32 = 90;
/// Retail DelayBetweenShots Min = 100 ms → 3 frames @ 30 FPS.
pub const SCUD_STORM_DELAY_BETWEEN_MIN_FRAMES: u32 = 3;
/// Retail DelayBetweenShots Max = 1000 ms → 30 frames @ 30 FPS.
pub const SCUD_STORM_DELAY_BETWEEN_MAX_FRAMES: u32 = 30;
/// Retail `LargePoisonFieldWeapon` PrimaryDamage (OCL_PoisonFieldLarge residual).
pub const SCUD_STORM_POISON_DAMAGE_PER_TICK: f32 = 15.0;
/// Retail `LargePoisonFieldWeaponUpgraded` PrimaryDamage (OCL_PoisonFieldUpgradedLarge).
pub const SCUD_STORM_POISON_DAMAGE_PER_TICK_UPGRADED: f32 = 25.0;
/// Retail `LargePoisonFieldWeapon` PrimaryDamageRadius.
pub const SCUD_STORM_POISON_RADIUS: f32 = 140.0;
/// Retail LargePoisonField DelayBetweenShots 500 ms → 15 frames.
pub const SCUD_STORM_POISON_TICK_INTERVAL_FRAMES: u32 = 15;
/// Retail PoisonFieldLarge LifetimeUpdate Min/MaxLifetime = 45000 ms → 1350 frames.
pub const SCUD_STORM_POISON_DURATION_FRAMES: u32 = 1350;
/// Retail OCL_PoisonFieldLarge CreateObject residual.
pub const SCUD_POISON_OBJECT_NAME: &str = "PoisonFieldLarge";
/// Retail PoisonFieldLarge MaxHealth residual.
pub const SCUD_POISON_FIELD_MAX_HEALTH: f32 = 100.0;
/// Retail OCL_PoisonFieldUpgradedLarge CreateObject residual.
pub const SCUD_POISON_UPGRADED_OBJECT_NAME: &str = "PoisonFieldUpgradedLarge";
/// Retail PoisonFieldUpgradedLarge MaxHealth residual.
pub const SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH: f32 = 120.0;
/// Residual ambient cue for ScudStorm poison pools.
pub const SCUD_STORM_POISON_AUDIO: &str = "ToxicPoolAmbientLoop";
/// Retail player upgrade selecting ScudStormDamageWeaponUpgraded / UpgradedLarge poison.
pub const UPGRADE_GLA_ANTHRAX_BETA_SCUD: &str = "Upgrade_GLAAnthraxBeta";

/// Secondary damage for ScudStorm residual (base 150 / Anthrax Beta 200).
pub fn scud_storm_secondary_damage(anthrax_beta: bool) -> f32 {
    if anthrax_beta {
        SCUD_STORM_SECONDARY_DAMAGE_UPGRADED
    } else {
        SCUD_STORM_SECONDARY_DAMAGE
    }
}

/// Poison tick damage for ScudStorm residual (base 15 / Anthrax Beta 25).
pub fn scud_storm_poison_damage_per_tick(anthrax_beta: bool) -> f32 {
    if anthrax_beta {
        SCUD_STORM_POISON_DAMAGE_PER_TICK_UPGRADED
    } else {
        SCUD_STORM_POISON_DAMAGE_PER_TICK
    }
}
/// Alias for LargePoisonFieldWeaponUpgraded PrimaryDamage residual.
pub const SCUD_STORM_POISON_DAMAGE_UPGRADED: f32 = SCUD_STORM_POISON_DAMAGE_PER_TICK_UPGRADED;
/// Retail `Chem_ScudStormDamageWeaponGamma` PrimaryDamage.
pub const SCUD_STORM_PRIMARY_DAMAGE_GAMMA: f32 = 550.0;
/// Residual ambient cue for upgraded anthrax poison pools.
pub const SCUD_STORM_POISON_AUDIO_UPGRADED: &str = "AnthraxPoolAmbientLoop";
/// Retail ScudStorm FireFX residual (per-missile launch).
pub const SCUD_STORM_FIRE_FX: &str = "WeaponFX_ScudStormMissile";
/// Retail ScudStorm ProjectileDetonationFX residual.
pub const SCUD_STORM_DETONATION_FX: &str = "ScudStormMissileDetonation";
/// Retail WeaponLaunchBone PRIMARY residual.
pub const SCUD_STORM_LAUNCH_BONE: &str = "WeaponA";
/// Retail ParticleSysBone Chem goo residual template.
pub const SCUD_STORM_CHEM_FX_PARTICLE: &str = "ScudStormBuildingGoo";
/// Retail Chem FXBone count (FXBone01..FXBone03).
pub const SCUD_STORM_CHEM_FX_BONE_COUNT: u32 = 3;
/// Retail Chem FXBone base name residual.
pub const SCUD_STORM_CHEM_FX_BONE_NAME: &str = "FXBone";

// --- ScudStormMissile loft residual (MissileAIUpdate / HeightDie / Locomotor) ---
/// Retail ProjectileObject residual name.
pub const SCUD_STORM_MISSILE_OBJECT: &str = "ScudStormMissile";
/// Retail W3DModelDraw model residual (`UBScudStrm_M`).
pub const SCUD_STORM_MISSILE_MODEL: &str = "UBScudStrm_M";
/// Retail MissileAIUpdate TryToFollowTarget residual (ballistic loft, no chase).
pub const SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET: bool = false;
/// Retail MissileAIUpdate FuelLifetime residual (0 = infinite).
pub const SCUD_STORM_MISSILE_FUEL_LIFETIME: u32 = 0;
/// Retail MissileAIUpdate InitialVelocity residual (dist/sec).
pub const SCUD_STORM_MISSILE_INITIAL_VELOCITY: f32 = 0.0;
/// Retail MissileAIUpdate DistanceToTravelBeforeTurning residual.
pub const SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING: f32 = 500.0;
/// Retail MissileAIUpdate DistanceToTargetBeforeDiving residual.
pub const SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING: f32 = 200.0;
/// Retail MissileAIUpdate IgnitionFX residual.
pub const SCUD_STORM_MISSILE_IGNITION_FX: &str = "FX_ScudStormIgnition";
/// Retail ScudStormWeapon FireSound residual.
pub const SCUD_STORM_MISSILE_LAUNCH_SOUND: &str = "ScudStormLaunch";
/// Retail ScudStormWeapon ProjectileExhaust residual.
pub const SCUD_STORM_MISSILE_EXHAUST: &str = "ScudMissileExhaust";
/// Retail HeightDieUpdate TargetHeight residual (structures included).
pub const SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET: f32 = 15.0;
/// Retail HeightDieUpdate InitialDelay residual (1000 ms → 30 frames).
pub const SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES: u32 = (1000 * 30) / 1000;
/// Retail HeightDieUpdate OnlyWhenMovingDown residual.
pub const SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN: bool = true;
/// Retail HeightDieUpdate SnapToGroundOnDeath residual.
pub const SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH: bool = true;
/// Retail HeightDieUpdate TargetHeightIncludesStructures residual.
pub const SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES: bool = true;
/// Retail SCUDStormMissileLocomotor Speed residual (dist/sec).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_SPEED: f32 = 300.0;
/// Retail SCUDStormMissileLocomotor SpeedDamaged residual (dist/sec).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_SPEED_DAMAGED: f32 = 200.0;
/// Retail SCUDStormMissileLocomotor MinSpeed residual (dist/sec).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_MIN_SPEED: f32 = 100.0;
/// Retail SCUDStormMissileLocomotor Acceleration residual (dist/sec²).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_ACCEL: f32 = 675.0;
/// Retail SCUDStormMissileLocomotor TurnRate residual (degrees/sec).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_TURN_RATE: f32 = 540.0;
/// Retail SCUDStormMissileLocomotor MaxThrustAngle residual (degrees).
pub const SCUD_STORM_MISSILE_LOCOMOTOR_MAX_THRUST_ANGLE: f32 = 45.0;
/// Retail SCUDStormMissileLocomotor ThrustRoll residual.
pub const SCUD_STORM_MISSILE_THRUST_ROLL: f32 = 0.06;
/// Retail SCUDStormMissileLocomotor ThrustWobbleRate residual.
pub const SCUD_STORM_MISSILE_THRUST_WOBBLE_RATE: f32 = 0.008;
/// Retail SCUDStormMissileLocomotor ThrustMinWobble residual.
pub const SCUD_STORM_MISSILE_THRUST_MIN_WOBBLE: f32 = -0.040;
/// Retail SCUDStormMissileLocomotor ThrustMaxWobble residual.
pub const SCUD_STORM_MISSILE_THRUST_MAX_WOBBLE: f32 = 0.040;
/// Retail SCUDStormMissileLocomotor CloseEnoughDist3D residual.
pub const SCUD_STORM_MISSILE_CLOSE_ENOUGH_DIST_3D: bool = true;
/// Retail SCUDStormMissileLocomotor PreferredHeight residual.
pub const SCUD_STORM_MISSILE_PREFERRED_HEIGHT: f32 = 240.0;
/// Retail SCUDStormMissileLocomotor PreferredHeightDamping residual.
pub const SCUD_STORM_MISSILE_PREFERRED_HEIGHT_DAMPING: f32 = 0.7;
/// Retail PhysicsBehavior Mass residual.
pub const SCUD_STORM_MISSILE_MASS: f32 = 500.0;
/// Retail ActiveBody MaxHealth residual.
pub const SCUD_STORM_MISSILE_MAX_HEALTH: f32 = 10000.0;
/// Retail GeometryMajorRadius residual.
pub const SCUD_STORM_MISSILE_GEOMETRY_RADIUS: f32 = 7.0;
/// Retail GeometryHeight residual.
pub const SCUD_STORM_MISSILE_GEOMETRY_HEIGHT: f32 = 30.0;
/// Retail GeometryIsSmall residual.
pub const SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL: bool = true;
/// Retail Geometry type residual.
pub const SCUD_STORM_MISSILE_GEOMETRY: &str = "Cylinder";
/// Retail VisionRange residual.
pub const SCUD_STORM_MISSILE_VISION_RANGE: f32 = 300.0;
/// Retail ShroudClearingRange residual.
pub const SCUD_STORM_MISSILE_SHROUD_CLEARING_RANGE: f32 = 0.0;
/// Retail KindOf residual (PROJECTILE).
pub const SCUD_STORM_MISSILE_KIND_OF: &str = "PROJECTILE";
/// Retail ArmorSet Armor residual.
pub const SCUD_STORM_MISSILE_ARMOR: &str = "ProjectileArmor";
/// Retail TransportSlotCount residual.
pub const SCUD_STORM_MISSILE_TRANSPORT_SLOT_COUNT: u32 = 10;
/// Retail SpecialPowerCompletionDie template residual.
pub const SCUD_STORM_MISSILE_SPECIAL_POWER: &str = "SuperweaponScudStorm";
/// Retail ActiveBody InitialHealth residual.
pub const SCUD_STORM_MISSILE_INITIAL_HEALTH: f32 = 10000.0;
/// Retail EditorSorting residual.
pub const SCUD_STORM_MISSILE_EDITOR_SORTING: &str = "SYSTEM";
/// Retail W3DModelDraw OkToChangeModelColor residual.
pub const SCUD_STORM_MISSILE_OK_TO_CHANGE_MODEL_COLOR: bool = true;
/// Retail DAMAGED/REALLYDAMAGED/RUBBLE model residual.
pub const SCUD_STORM_MISSILE_DAMAGED_MODEL: &str = "NONE";
/// Retail FireWeaponWhenDeadBehavior base DeathWeapon residual.
pub const SCUD_STORM_MISSILE_DEATH_WEAPON_BASE: &str = "ScudStormDamageWeapon";
/// Retail FireWeaponWhenDeadBehavior upgraded DeathWeapon residual.
pub const SCUD_STORM_MISSILE_DEATH_WEAPON_UPGRADED: &str = "ScudStormDamageWeaponUpgraded";
/// Retail FireWeaponWhenDead base ConflictsWith residual.
pub const SCUD_STORM_MISSILE_DEATH_CONFLICTS_WITH: &str = "Upgrade_GLAAnthraxBeta";
/// Retail FireWeaponWhenDead upgraded TriggeredBy residual.
pub const SCUD_STORM_MISSILE_DEATH_TRIGGERED_BY: &str = "Upgrade_GLAAnthraxBeta";
/// Retail FireWeaponWhenDead base StartsActive residual.
pub const SCUD_STORM_MISSILE_DEATH_BASE_STARTS_ACTIVE: bool = true;
/// Retail FireWeaponWhenDead upgraded StartsActive residual.
pub const SCUD_STORM_MISSILE_DEATH_UPGRADED_STARTS_ACTIVE: bool = false;
/// Retail SCUDStormMissileLocomotor Surfaces residual.
pub const SCUD_STORM_MISSILE_LOCOMOTOR_SURFACES: &str = "AIR";
/// Retail SCUDStormMissileLocomotor Appearance residual.
pub const SCUD_STORM_MISSILE_LOCOMOTOR_APPEARANCE: &str = "THRUST";
/// Retail SCUDStormMissileLocomotor AllowAirborneMotiveForce residual.
pub const SCUD_STORM_MISSILE_LOCOMOTOR_ALLOW_AIRBORNE_MOTIVE: bool = true;
/// Retail SCUDStormMissileLocomotor Braking residual.
pub const SCUD_STORM_MISSILE_LOCOMOTOR_BRAKING: f32 = 0.0;
/// Retail Locomotor SET_NORMAL template name residual.
pub const SCUD_STORM_MISSILE_LOCOMOTOR_NAME: &str = "SCUDStormMissileLocomotor";
/// Retail DestroyDie module residual (empty module present on ScudStormMissile).
pub const SCUD_STORM_MISSILE_DESTROY_DIE: bool = true;
/// Retail ArmorSet DamageFX residual (`None`).
pub const SCUD_STORM_MISSILE_DAMAGE_FX: &str = "None";
/// Retail ScudStormDamageWeapon FireOCL residual.
pub const SCUD_STORM_MISSILE_DEATH_FIRE_OCL_BASE: &str = "OCL_PoisonFieldLarge";
/// Retail ScudStormDamageWeaponUpgraded FireOCL residual.
pub const SCUD_STORM_MISSILE_DEATH_FIRE_OCL_UPGRADED: &str = "OCL_PoisonFieldUpgradedLarge";
/// Retail ScudStormDamageWeapon DamageType residual.
pub const SCUD_STORM_MISSILE_DEATH_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail ScudStormDamageWeapon DeathType residual.
pub const SCUD_STORM_MISSILE_DEATH_DEATH_TYPE: &str = "EXPLODED";
/// Retail ScudStormDamageWeapon WeaponSpeed residual (dist/sec).
pub const SCUD_STORM_MISSILE_DEATH_WEAPON_SPEED: f32 = 600.0;
/// Retail ScudStormDamageWeapon AttackRange residual.
pub const SCUD_STORM_MISSILE_DEATH_ATTACK_RANGE: f32 = 200.0;
/// Retail ScudStormDamageWeapon FireFX residual (detonation FX name).
pub const SCUD_STORM_MISSILE_DEATH_FIRE_FX: &str = "ScudStormMissileDetonation";
/// Retail ScudStormDamageWeapon RadiusDamageAffects residual.
pub const SCUD_STORM_MISSILE_DEATH_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Retail ScudStormDamageWeapon DelayBetweenShots residual (msec; 0 instant).
pub const SCUD_STORM_MISSILE_DEATH_DELAY_BETWEEN_SHOTS_MS: u32 = 0;
/// Retail ScudStormDamageWeapon ClipSize residual (0 == infinite).
pub const SCUD_STORM_MISSILE_DEATH_CLIP_SIZE: u32 = 0;
/// Retail ScudStormDamageWeapon ClipReloadTime residual (msec).
pub const SCUD_STORM_MISSILE_DEATH_CLIP_RELOAD_TIME_MS: u32 = 0;
/// Retail ScudStormWeapon ClipSize residual (alias of missile count).
pub const SCUD_STORM_CLIP_SIZE: u32 = SCUD_STORM_MISSILE_COUNT;
/// Retail ScudStormWeapon ClipReloadTime residual (msec; pad sink time).
pub const SCUD_STORM_CLIP_RELOAD_TIME_MS: u32 = 10000;
/// Retail ScudStormWeapon ClipReloadTime 10000 ms → 300 frames @ 30 FPS.
pub const SCUD_STORM_CLIP_RELOAD_FRAMES: u32 = (SCUD_STORM_CLIP_RELOAD_TIME_MS * 30) / 1000;
/// Retail ScudStormWeapon AutoReloadsClip residual.
pub const SCUD_STORM_AUTO_RELOADS_CLIP: bool = true;
/// Retail ScudStormWeapon AcceptableAimDelta residual (degrees).
pub const SCUD_STORM_ACCEPTABLE_AIM_DELTA: f32 = 180.0;
/// Retail ScudStormWeapon ProjectileCollidesWith residual.
pub const SCUD_STORM_PROJECTILE_COLLIDES_WITH: &str = "STRUCTURES";
/// Retail ScudStormWeapon ProjectileObject residual.
pub const SCUD_STORM_PROJECTILE_OBJECT: &str = "ScudStormMissile";
/// Retail ScudStormWeapon DelayBetweenShots Min residual (msec).
pub const SCUD_STORM_DELAY_BETWEEN_MIN_MS: u32 = 100;
/// Retail ScudStormWeapon DelayBetweenShots Max residual (msec).
pub const SCUD_STORM_DELAY_BETWEEN_MAX_MS: u32 = 1000;
/// Retail ScudStormWeapon ScatterTarget table entry count residual.
pub const SCUD_STORM_SCATTER_TARGET_COUNT: u32 = 9;
/// Retail ScudStormWeapon PrimaryDamage residual (0 — unused / special launch weapon).
pub const SCUD_STORM_WEAPON_PRIMARY_DAMAGE: f32 = 0.0;
/// Retail ScudStormWeapon PrimaryDamageRadius residual (0 — unused).
pub const SCUD_STORM_WEAPON_PRIMARY_RADIUS: f32 = 0.0;
/// Retail ScudStormWeapon AttackRange residual (unused special).
pub const SCUD_STORM_WEAPON_ATTACK_RANGE: f32 = 999_999.0;
/// Retail ScudStormWeapon DamageType residual.
pub const SCUD_STORM_WEAPON_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail ScudStormWeapon DeathType residual.
pub const SCUD_STORM_WEAPON_DEATH_TYPE: &str = "EXPLODED";
/// Retail ScudStormWeapon WeaponSpeed residual (dist/sec; unused special).
pub const SCUD_STORM_WEAPON_SPEED: f32 = 99_999.0;
/// Retail ScudStormWeapon ScatterRadius residual (0; table uses ScatterTargetScalar).
pub const SCUD_STORM_SCATTER_RADIUS: f32 = 0.0;
/// Retail ScudStormWeapon PreAttackType residual.
pub const SCUD_STORM_PRE_ATTACK_TYPE: &str = "PER_CLIP";
/// Retail ScudStormWeapon PreAttackDelay residual (msec).
pub const SCUD_STORM_PRE_ATTACK_DELAY_MS: u32 = 3000;
/// Retail MissileAIUpdate IgnitionDelay residual (unset → default 0 frames).
pub const SCUD_STORM_MISSILE_IGNITION_DELAY_FRAMES: u32 = 0;
/// Retail MissileAIUpdate UseWeaponSpeed residual (default false).
pub const SCUD_STORM_MISSILE_USE_WEAPON_SPEED: bool = false;
/// Retail MissileAIUpdate DetonateOnNoFuel residual (default false).
pub const SCUD_STORM_MISSILE_DETONATE_ON_NO_FUEL: bool = false;
/// Retail MissileAIUpdate DistanceToTargetForLock residual (default 75).
pub const SCUD_STORM_MISSILE_DISTANCE_FOR_LOCK: f32 = 75.0;
/// Retail MissileAIUpdate DistanceScatterWhenJammed residual (default 75).
pub const SCUD_STORM_MISSILE_DISTANCE_SCATTER_WHEN_JAMMED: f32 = 75.0;
/// Retail MissileAIUpdate DetonateCallsKill residual (default false).
pub const SCUD_STORM_MISSILE_DETONATE_CALLS_KILL: bool = false;
/// Retail MissileAIUpdate KillSelfDelay residual (default 3 frames).
pub const SCUD_STORM_MISSILE_KILL_SELF_DELAY_FRAMES: u32 = 3;
/// Retail ScudStormWeapon ProjectileDetonationFX residual.
pub const SCUD_STORM_PROJECTILE_DETONATION_FX: &str = "ScudStormMissileDetonation";
/// Retail ScudStormWeapon RadiusDamageAffects residual (special launch weapon).
pub const SCUD_STORM_WEAPON_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";

/// Residual ScudStormMissile loft phase (MissileAIUpdate / Locomotor path).
///
/// Host residual tracks phase honesty without a full ThingFactory Object flight sim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ScudMissileLoftPhase {
    /// Initial ballistic loft toward PreferredHeight residual.
    #[default]
    Loft = 0,
    /// Past DistanceToTravelBeforeTurning residual (begin course correction).
    Turn = 1,
    /// Within DistanceToTargetBeforeDiving residual (dive to HeightDie target).
    Dive = 2,
    /// HeightDieUpdate residual (below TargetHeight after InitialDelay).
    HeightDie = 3,
}

impl ScudMissileLoftPhase {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Retail spawn height residual (`PreferredHeight` above surface).
///
/// Fail-closed: not full terrain surface height sample / StartAtPreferredHeight
/// OCL nugget Object path (host residual assumes flat ground surfaceHt = 0).
#[inline]
pub fn scud_missile_spawn_height() -> f32 {
    SCUD_STORM_MISSILE_PREFERRED_HEIGHT
}

/// Retail Locomotor PreferredHeight spring residual for one logic frame.
///
/// C++ `Locomotor::locoUpdate_moveTowards` (when preferred height set):
/// ```text
/// localGoal.z = preferredHeight + surfaceHt;
/// delta = (localGoal.z - pos.z) * PreferredHeightDamping;
/// localGoal.z = pos.z + delta;
/// ```
/// Host residual: `new = current + (preferred - current) * damping`.
#[inline]
pub fn scud_missile_preferred_height_spring(current_height: f32) -> f32 {
    let preferred = SCUD_STORM_MISSILE_PREFERRED_HEIGHT;
    let damping = SCUD_STORM_MISSILE_PREFERRED_HEIGHT_DAMPING;
    current_height + (preferred - current_height) * damping
}

/// Sample PreferredHeight spring residual after `frames` logic steps from `start_height`.
#[inline]
pub fn scud_missile_preferred_height_after_frames(start_height: f32, frames: u32) -> f32 {
    let mut h = start_height;
    for _ in 0..frames {
        h = scud_missile_preferred_height_spring(h);
    }
    h
}

/// Residual loft phase for a ScudStormMissile given travel distances.
///
/// Order (retail MissileAIUpdate): loft → turn after DistanceBeforeTurning →
/// dive when within DistanceBeforeDiving of target → HeightDie near ground.
#[inline]
pub fn scud_missile_loft_phase(
    distance_traveled: f32,
    distance_to_target: f32,
    current_height: f32,
) -> ScudMissileLoftPhase {
    if current_height <= SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET && distance_traveled > 0.0 {
        return ScudMissileLoftPhase::HeightDie;
    }
    if distance_to_target <= SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING {
        return ScudMissileLoftPhase::Dive;
    }
    if distance_traveled >= SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING {
        return ScudMissileLoftPhase::Turn;
    }
    ScudMissileLoftPhase::Loft
}

/// Horizontal locomotor step residual per logic frame (Speed / FPS).
#[inline]
pub fn scud_missile_speed_per_frame() -> f32 {
    SCUD_STORM_MISSILE_LOCOMOTOR_SPEED / SP_LOGIC_FPS
}

/// Host residual ballistic flight sample after `frames` from launch.
///
/// Advances horizontal position toward target at locomotor speed, applies
/// PreferredHeight spring while not diving, then dives toward HeightDie target
/// once within DistanceBeforeDiving. Fail-closed: not full Physics motive force
/// / turn-rate matrix / ThingFactory Object path.
///
/// Returns (position, distance_traveled, distance_to_target, phase).
pub fn scud_missile_ballistic_sample(
    launch: Vec3,
    target: Vec3,
    frames: u32,
) -> (Vec3, f32, f32, ScudMissileLoftPhase) {
    let mut pos = Vec3::new(launch.x, scud_missile_spawn_height(), launch.z);
    let mut traveled = 0.0f32;
    let step = scud_missile_speed_per_frame();
    let mut prev_height = pos.y;
    let mut moving_down = false;

    for _ in 0..frames {
        let to_target = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
        let dist_h = (to_target.x * to_target.x + to_target.z * to_target.z).sqrt();
        let phase = scud_missile_loft_phase(traveled, dist_h, pos.y);
        if phase == ScudMissileLoftPhase::HeightDie {
            break;
        }
        // Horizontal advance toward target (MissileAI move-to-position residual).
        if dist_h > f32::EPSILON {
            let dir_x = to_target.x / dist_h;
            let dir_z = to_target.z / dist_h;
            let advance = step.min(dist_h);
            pos.x += dir_x * advance;
            pos.z += dir_z * advance;
            traveled += advance;
        }
        let dist_after = {
            let dx = target.x - pos.x;
            let dz = target.z - pos.z;
            (dx * dx + dz * dz).sqrt()
        };
        // Height: spring toward PreferredHeight unless diving / height-die.
        if dist_after <= SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING
            || phase == ScudMissileLoftPhase::Dive
        {
            // Dive residual: ignore PreferredHeight, sink toward HeightDie target.
            let dive_step = step.max(1.0);
            pos.y = (pos.y - dive_step).max(SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET * 0.5);
        } else {
            pos.y = scud_missile_preferred_height_spring(pos.y);
        }
        moving_down = pos.y < prev_height;
        prev_height = pos.y;
    }

    let dist_to = {
        let dx = target.x - pos.x;
        let dz = target.z - pos.z;
        (dx * dx + dz * dz).sqrt()
    };
    // OnlyWhenMovingDown residual: HeightDie only when descending.
    let phase = if pos.y <= SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET
        && traveled > 0.0
        && (moving_down || SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN)
    {
        ScudMissileLoftPhase::HeightDie
    } else {
        scud_missile_loft_phase(traveled, dist_to, pos.y)
    };
    // SnapToGroundOnDeath residual: snap Y to surface when HeightDie.
    if phase == ScudMissileLoftPhase::HeightDie && SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH {
        pos.y = 0.0;
    }
    (pos, traveled, dist_to, phase)
}

/// Retail ScatterTarget table (C++ X/Y horizontal), scaled by ScatterTargetScalar.
/// Host maps C++ X → X, C++ Y → Z.
pub const SCUD_STORM_SCATTER_TARGETS: [(f32, f32); 9] = [
    (0.000, 0.133),
    (0.133, -0.200),
    (-0.067, 0.667),
    (0.300, 0.300),
    (0.767, 0.000),
    (0.500, -0.567),
    (-0.333, -0.800),
    (-0.600, -0.1333),
    (-0.567, 0.433),
];

// --- ScudStorm anthrax-upgrade residual (ScudStormDamageWeaponUpgraded / Chem_Gamma) ---

/// Residual ScudStorm anthrax warhead tier.
///
/// Retail:
/// - Base `ScudStormDamageWeapon`: Primary **500** / Secondary **150** + LargePoison **15**
/// - Anthrax Beta `ScudStormDamageWeaponUpgraded`: Primary **500** / Secondary **200**
///   + LargePoison upgraded **25**
/// - Chem Gamma `Chem_ScudStormDamageWeaponGamma`: Primary **550** / Secondary **200**
///   + LargePoison gamma **25**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ScudStormAnthraxTier {
    /// Unupgraded ScudStormDamageWeapon residual.
    #[default]
    Base,
    /// Upgrade_GLAAnthraxBeta residual (Secondary 200 + poison 25).
    AnthraxBeta,
    /// Chem_Upgrade_GLAAnthraxGamma residual (Primary 550 + Secondary 200 + poison 25).
    AnthraxGamma,
}

impl ScudStormAnthraxTier {
    /// Primary blast damage residual for this anthrax tier.
    pub fn primary_damage(self) -> f32 {
        match self {
            ScudStormAnthraxTier::AnthraxGamma => SCUD_STORM_PRIMARY_DAMAGE_GAMMA,
            _ => SCUD_STORM_PRIMARY_DAMAGE,
        }
    }

    /// Secondary blast damage residual for this anthrax tier.
    pub fn secondary_damage(self) -> f32 {
        match self {
            ScudStormAnthraxTier::Base => SCUD_STORM_SECONDARY_DAMAGE,
            ScudStormAnthraxTier::AnthraxBeta | ScudStormAnthraxTier::AnthraxGamma => {
                SCUD_STORM_SECONDARY_DAMAGE_UPGRADED
            }
        }
    }

    /// LargePoisonField residual damage per tick for this anthrax tier.
    pub fn poison_damage_per_tick(self) -> f32 {
        match self {
            ScudStormAnthraxTier::Base => SCUD_STORM_POISON_DAMAGE_PER_TICK,
            ScudStormAnthraxTier::AnthraxBeta | ScudStormAnthraxTier::AnthraxGamma => {
                SCUD_STORM_POISON_DAMAGE_UPGRADED
            }
        }
    }

    /// Whether residual spawns upgraded (Beta/Gamma) LargePoison field stats.
    pub fn is_upgraded(self) -> bool {
        !matches!(self, ScudStormAnthraxTier::Base)
    }

    /// OCL CreateObject template residual for this anthrax tier.
    pub fn poison_object_name(self) -> &'static str {
        if self.is_upgraded() {
            SCUD_POISON_UPGRADED_OBJECT_NAME
        } else {
            SCUD_POISON_OBJECT_NAME
        }
    }

    /// MaxHealth residual for the poison field object of this tier.
    pub fn poison_field_max_health(self) -> f32 {
        if self.is_upgraded() {
            SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH
        } else {
            SCUD_POISON_FIELD_MAX_HEALTH
        }
    }

    /// Select highest anthrax tier from unlocked science/upgrade name list.
    pub fn highest_from_upgrades<'a, I>(upgrades: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut tier = ScudStormAnthraxTier::Base;
        for name in upgrades {
            let n: String = name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect();
            if n.contains("anthraxgamma") || n.contains("chem_upgrade_glaanthraxgamma") {
                return ScudStormAnthraxTier::AnthraxGamma;
            }
            if n.contains("anthraxbeta") || n.contains("upgrade_glaanthraxbeta") {
                tier = ScudStormAnthraxTier::AnthraxBeta;
            }
            // Chem general ScudStorm residual defaults to gamma warhead when
            // source template / science mentions Chem Scud Storm.
            if n.contains("chem") && n.contains("scudstorm") {
                return ScudStormAnthraxTier::AnthraxGamma;
            }
        }
        tier
    }
}
