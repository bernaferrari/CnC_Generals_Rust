//! Spectre gunship / gattling / howitzer residual constants and stage helpers.
use super::types::*;
// --- Spectre Gunship orbit residual (retail SpectreHowitzerGun / OrbitTime) ---

/// Retail `SpectreHowitzerGun` PrimaryDamage (orbit residual tick).
/// Fail-closed vs full gattling-strafe + howitzer projectile + random offset.
pub const SPECTRE_ORBIT_DAMAGE_PER_TICK: f32 = 80.0;
/// Retail `SpectreGunshipUpdate` AttackAreaRadius / RadiusCursorRadius.
pub const SPECTRE_ORBIT_RADIUS: f32 = 200.0;
/// Retail HowitzerFiringRate residual (msec).
pub const SPECTRE_HOWITZER_FIRING_RATE_MS: u32 = 300;
/// Retail HowitzerFiringRate = 300 ms → 9 frames @ 30 FPS.
pub const SPECTRE_ORBIT_TICK_INTERVAL_FRAMES: u32 = 9;
/// Retail OrbitTime = 15000 ms @ 30 FPS.
pub const SPECTRE_ORBIT_DURATION_FRAMES: u32 = 450;
/// Residual ambient cue for active Spectre orbit (`SpectreGunshipAmbientLoop`).
pub const SPECTRE_ORBIT_AUDIO: &str = "SpectreGunshipAmbientLoop";
/// Retail `SpectreHowitzerGun` PrimaryDamageRadius (howitzer blast residual).
pub const SPECTRE_HOWITZER_RADIUS: f32 = 25.0;
/// Retail `SpectreGunshipUpdate` RandomOffsetForHowitzer residual.
pub const SPECTRE_HOWITZER_RANDOM_OFFSET: f32 = 20.0;

// --- Wave 73: SpectreGunship orbit residual pack deepen ---

/// Retail `SpectreGunshipUpdate` HowitzerFollowLag residual (msec).
/// How long after gattling acquires target before howitzer may fire same.
pub const SPECTRE_HOWITZER_FOLLOW_LAG_MS: u32 = 400;
/// HowitzerFollowLag 400ms → 12 frames @ 30 FPS.
pub const SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES: u32 = 12;
/// Retail `SpectreGunshipUpdate` GunshipOrbitRadius residual (flight circle).
/// Distinct from AttackAreaRadius **200** (damage/cursor circle).
pub const SPECTRE_GUNSHIP_ORBIT_RADIUS: f32 = 250.0;
/// Retail `SpectreGunshipUpdate` TargetingReticleRadius residual.
pub const SPECTRE_TARGETING_RETICLE_RADIUS: f32 = 25.0;
/// C++ `AttackAreaRadius - TargetingReticleRadius` override clamp radius.
pub const SPECTRE_OVERRIDE_CONSTRAINT_RADIUS: f32 =
    SPECTRE_ORBIT_RADIUS - SPECTRE_TARGETING_RETICLE_RADIUS;

/// Retail `SpectreGunshipUpdate` StrafingIncrement residual (gattling step).
pub const SPECTRE_STRAFING_INCREMENT: f32 = 20.0;
/// Retail `SpectreGunshipUpdate` OrbitInsertionSlope residual.
pub const SPECTRE_ORBIT_INSERTION_SLOPE: f32 = 0.7;
/// Retail `SpectreGunshipUpdate` GattlingStrafeFXParticleSystem residual.
pub const SPECTRE_GATTLING_STRAFE_FX: &str = "SpectreGattlingArmsSmoke";
/// Retail AttackAreaDecal Texture residual.
pub const SPECTRE_ATTACK_AREA_DECAL_TEXTURE: &str = "SCCSpecTarg";
/// Retail TargetingReticleDecal Texture residual.
pub const SPECTRE_TARGETING_RETICLE_DECAL_TEXTURE: &str = "SCCSpecRet";
/// Retail AttackAreaDecal / TargetingReticleDecal Color residual (RGBA 0..255).
pub const SPECTRE_DECAL_COLOR: (u8, u8, u8, u8) = (127, 177, 222, 255);
/// Retail AttackAreaDecal OpacityThrobTime residual (msec).
pub const SPECTRE_ATTACK_AREA_DECAL_THROB_MS: u32 = 1500;
/// Retail TargetingReticleDecal OpacityThrobTime residual (msec).
pub const SPECTRE_TARGETING_RETICLE_DECAL_THROB_MS: u32 = 300;
/// Retail SuperweaponSpectreGunship ReloadTime residual (msec).
pub const SPECTRE_RELOAD_MS: u32 = 240_000;
/// SuperweaponSpectreGunship ReloadTime 240000ms → 7200 frames.
pub const SPECTRE_RELOAD_FRAMES: u32 = 7_200;
/// Retail AirF_SuperweaponSpectreGunship ReloadTime residual (msec).
pub const SPECTRE_AIRF_RELOAD_MS: u32 = 180_000;
/// AirF Spectre ReloadTime 180000ms → 5400 frames.
pub const SPECTRE_AIRF_RELOAD_FRAMES: u32 = 5_400;
/// Retail SuperweaponSpectreGunship ViewObjectDuration residual (msec).
pub const SPECTRE_VIEW_OBJECT_DURATION_MS: u32 = 30_000;
/// ViewObjectDuration 30000ms → 900 frames.
pub const SPECTRE_VIEW_OBJECT_DURATION_FRAMES: u32 = 900;
/// Retail SuperweaponSpectreGunship ViewObjectRange residual.
pub const SPECTRE_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SuperweaponSpectreGunship SpecialPowerTemplate name residual.
pub const SPECTRE_SPECIAL_POWER_TEMPLATE: &str = "SuperweaponSpectreGunship";
/// Retail AirF Spectre SpecialPowerTemplate name residual.
pub const SPECTRE_AIRF_SPECIAL_POWER_TEMPLATE: &str = "AirF_SuperweaponSpectreGunship";
/// Dual-weapon howitzer base interval frames (HowitzerFiringRate residual).
pub const SPECTRE_DUAL_HOWITZER_BASE_INTERVAL: u32 = SPECTRE_ORBIT_TICK_INTERVAL_FRAMES;
/// Dual-weapon howitzer MEAN interval (ROF 150% → floor(9/1.5)=6).
pub const SPECTRE_DUAL_HOWITZER_MEAN_INTERVAL: u32 = 6;
/// Dual-weapon howitzer FAST interval (ROF 200% → floor(9/2)=4).
pub const SPECTRE_DUAL_HOWITZER_FAST_INTERVAL: u32 = 4;
/// Dual-weapon gattling base interval frames (DelayBetweenShots 100ms → 3f).
pub const SPECTRE_DUAL_GATTLING_BASE_INTERVAL: u32 = 3;
/// Dual-weapon gattling MEAN interval (ROF 200% → floor(3/2)=1).
pub const SPECTRE_DUAL_GATTLING_MEAN_INTERVAL: u32 = 1;
/// Dual-weapon gattling FAST interval (ROF 300% → floor(3/3)=1).
pub const SPECTRE_DUAL_GATTLING_FAST_INTERVAL: u32 = 1;
/// Retail `SpectreGattlingGun` PrimaryDamage (single-target residual).
pub const SPECTRE_GATTLING_DAMAGE: f32 = 90.0;
/// Retail `SpectreGattlingGun` DelayBetweenShots = 100 ms → 3 frames @ 30 FPS.
/// Base interval (ContinuousFire Normal / ROF 100%).
pub const SPECTRE_GATTLING_TICK_INTERVAL_FRAMES: u32 = 3;
/// Retail ContinuousFireOne — consecutive shots needed before MEAN ROF residual.
pub const SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE: u32 = 1;
/// Retail ContinuousFireTwo — consecutive shots needed before FAST ROF residual.
pub const SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO: u32 = 2;
/// Retail WeaponBonus CONTINUOUS_FIRE_MEAN RATE_OF_FIRE **200%**.
pub const SPECTRE_GATTLING_ROF_MEAN: f32 = 2.0;
/// Retail WeaponBonus CONTINUOUS_FIRE_FAST RATE_OF_FIRE **300%**.
pub const SPECTRE_GATTLING_ROF_FAST: f32 = 3.0;
/// Residual honesty audio for gattling strafe residual.
pub const SPECTRE_GATTLING_AUDIO: &str = "SpectreGunshipGattlingWeapon";
/// Retail SpectreGattlingGun PrimaryDamageRadius residual (0 = intended victim only).
pub const SPECTRE_GATTLING_PRIMARY_RADIUS: f32 = 0.0;
/// Retail SpectreGattlingGun AttackRange residual.
pub const SPECTRE_GATTLING_ATTACK_RANGE: f32 = 2222.0;
/// Retail SpectreGattlingGun DamageType residual.
pub const SPECTRE_GATTLING_DAMAGE_TYPE: &str = "Gattling";
/// Retail SpectreGattlingGun DeathType residual.
pub const SPECTRE_GATTLING_DEATH_TYPE: &str = "NORMAL";
/// Retail SpectreGattlingGun WeaponSpeed residual (instant).
pub const SPECTRE_GATTLING_WEAPON_SPEED: f32 = 999_999.0;
/// Retail SpectreGattlingGun ProjectileObject residual (hitscan NONE).
pub const SPECTRE_GATTLING_PROJECTILE_OBJECT: &str = "NONE";
/// Retail SpectreGattlingGun FireFX residual.
pub const SPECTRE_GATTLING_FIRE_FX: &str = "WeaponFX_SpectreGattlingMuzzleFlash";
/// Retail SpectreGattlingGun VeterancyFireFX residual (HEROIC red tracers).
pub const SPECTRE_GATTLING_VETERANCY_FIRE_FX: &str =
    "WeaponFX_GattlingCannonMachineGunFireWithRedTracers";
/// Retail SpectreGattlingGun RadiusDamageAffects residual.
pub const SPECTRE_GATTLING_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Retail SpectreGattlingGun DelayBetweenShots residual (msec).
pub const SPECTRE_GATTLING_DELAY_BETWEEN_SHOTS_MS: u32 = 100;
/// Retail SpectreGattlingGun ClipSize residual (0 == infinite).
pub const SPECTRE_GATTLING_CLIP_SIZE: u32 = 0;
/// Retail SpectreGattlingGun ClipReloadTime residual (msec).
pub const SPECTRE_GATTLING_CLIP_RELOAD_TIME_MS: u32 = 0;
/// Retail SpectreGattlingGun AntiAirborneVehicle residual.
pub const SPECTRE_GATTLING_ANTI_AIRBORNE_VEHICLE: bool = false;
/// Retail SpectreGattlingGun AntiAirborneInfantry residual.
pub const SPECTRE_GATTLING_ANTI_AIRBORNE_INFANTRY: bool = false;
/// Retail SpectreGattlingGun AntiSmallMissile residual.
pub const SPECTRE_GATTLING_ANTI_SMALL_MISSILE: bool = false;
/// Retail SpectreGattlingGun AntiBallisticMissile residual.
pub const SPECTRE_GATTLING_ANTI_BALLISTIC_MISSILE: bool = false;
/// Retail SpectreGattlingGun AntiGround residual.
pub const SPECTRE_GATTLING_ANTI_GROUND: bool = true;
/// Retail VoiceRapidFire residual cue when ContinuousFire enters FAST
/// (`FiringTracker::speedUp` PerUnitSound "VoiceRapidFire"). Host residual:
/// honesty name for Spectre orbit when gattling/howitzer reaches FAST.
pub const SPECTRE_VOICE_RAPID_FIRE_AUDIO: &str = "SpectreGunshipVoiceRapidFire";

/// Residual Spectre gattling ContinuousFire stage (FiringTracker MEAN/FAST).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SpectreGattlingFireStage {
    /// Base DelayBetweenShots (ROF 100%).
    #[default]
    Normal,
    /// CONTINUOUS_FIRE_MEAN — ROF 200% residual.
    Mean,
    /// CONTINUOUS_FIRE_FAST — ROF 300% residual.
    Fast,
}

impl SpectreGattlingFireStage {
    /// Retail RATE_OF_FIRE multiplier for this continuous-fire stage.
    pub fn rate_of_fire(self) -> f32 {
        match self {
            SpectreGattlingFireStage::Normal => 1.0,
            SpectreGattlingFireStage::Mean => SPECTRE_GATTLING_ROF_MEAN,
            SpectreGattlingFireStage::Fast => SPECTRE_GATTLING_ROF_FAST,
        }
    }

    /// Tick interval frames: `floor(base_delay / ROF)` (C++ getDelayBetweenShots).
    pub fn tick_interval_frames(self) -> u32 {
        let base = SPECTRE_GATTLING_TICK_INTERVAL_FRAMES as f32;
        let rof = self.rate_of_fire().max(f32::EPSILON);
        ((base / rof).floor() as u32).max(1)
    }
}

/// Advance ContinuousFire stage after a gattling shot (FiringTracker residual).
///
/// Retail: ContinuousFireOne=1, ContinuousFireTwo=2 on `SpectreGattlingGun`.
/// - From Normal: consecutive > One → MEAN
/// - From Mean: consecutive > Two → FAST
/// - From Fast: stay FAST while consecutive holds (coast cool-down resets via
///   [`spectre_coast_spin_down`])
pub fn spectre_gattling_stage_after_shot(
    stage: SpectreGattlingFireStage,
    consecutive_shots: u32,
) -> SpectreGattlingFireStage {
    match stage {
        SpectreGattlingFireStage::Normal => {
            if consecutive_shots > SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE {
                SpectreGattlingFireStage::Mean
            } else {
                SpectreGattlingFireStage::Normal
            }
        }
        SpectreGattlingFireStage::Mean => {
            if consecutive_shots > SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO {
                SpectreGattlingFireStage::Fast
            } else {
                SpectreGattlingFireStage::Mean
            }
        }
        SpectreGattlingFireStage::Fast => SpectreGattlingFireStage::Fast,
    }
}

/// Retail ContinuousFireCoast residual for Spectre gattling / howitzer (both 2000 ms).
///
/// C++ FiringTracker: `m_frameToStartCooldown = possibleNextShotFrame + coast`.
/// When `now > m_frameToStartCooldown`, coolDown() zeros consecutive and clears
/// MEAN/FAST weapon-bonus flags.
pub const SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES: u32 = 60;

/// Next coast-until frame after a residual shot (next possible shot + coast).
///
/// Fail-closed: uses `current_frame + interval + coast` (not full
/// Weapon::getPossibleNextShotFrame).
pub fn spectre_coast_until_after_shot(current_frame: u32, interval_frames: u32) -> u32 {
    current_frame
        .saturating_add(interval_frames.max(1))
        .saturating_add(SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES)
}

/// Coast elapsed: spin down consecutive + fire level residual.
///
/// Returns `Some((0, 0))` when cool-down applies (consecutive cleared, level base).
/// Returns `None` while still within coast window (or coast never armed).
pub fn spectre_coast_spin_down(
    current_frame: u32,
    coast_until_frame: u32,
    fire_level: u8,
    consecutive: u32,
) -> Option<(u32, u8)> {
    if coast_until_frame == 0 || current_frame <= coast_until_frame {
        return None;
    }
    // Already cool and idle — nothing to clear.
    if fire_level == 0 && consecutive == 0 {
        return None;
    }
    // C++ coolDown: consecutive = 0, clear MEAN/FAST → base.
    Some((0, 0))
}

/// Alias residual ROF multipliers used by interval helpers.
pub const SPECTRE_GATTLING_MEAN_ROF_MULT: f32 = SPECTRE_GATTLING_ROF_MEAN;
pub const SPECTRE_GATTLING_FAST_ROF_MULT: f32 = SPECTRE_GATTLING_ROF_FAST;
/// Retail `SpectreHowitzerGun` ContinuousFireOne.
pub const SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE: u32 = 1;
/// Retail `SpectreHowitzerGun` ContinuousFireTwo.
pub const SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO: u32 = 2;
/// Retail WeaponBonus CONTINUOUS_FIRE_MEAN RATE_OF_FIRE 150% (howitzer).
pub const SPECTRE_HOWITZER_MEAN_ROF_MULT: f32 = 1.5;
/// Retail WeaponBonus CONTINUOUS_FIRE_FAST RATE_OF_FIRE 200% (howitzer).
pub const SPECTRE_HOWITZER_FAST_ROF_MULT: f32 = 2.0;

// --- SpectreHowitzerShell projectile residual (WeaponObjects.ini) ---

/// Retail `SpectreHowitzerGun` ProjectileObject name honesty.
pub const SPECTRE_HOWITZER_SHELL_OBJECT: &str = "SpectreHowitzerShell";
/// Retail `SpectreHowitzerGun` WeaponSpeed (dist/sec residual).
pub const SPECTRE_HOWITZER_WEAPON_SPEED: f32 = 999.0;
/// Retail `SpectreHowitzerGun` FireFX residual.
pub const SPECTRE_HOWITZER_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracer";
/// Retail `SpectreHowitzerGun` ProjectileDetonationFX residual.
pub const SPECTRE_HOWITZER_DETONATION_FX: &str = "FX_SpectreHowitzerExplosion";
/// Retail `SpectreHowitzerGun` FireSound residual.
pub const SPECTRE_HOWITZER_FIRE_SOUND: &str = "StrategyCenter_ArtilleryRound";
/// Retail HeightDieUpdate InitialDelay = 1000 ms → 30 frames @ 30 FPS.
/// Shell cannot explode on the pad for the first second residual.
pub const SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES: u32 = (1000 * 30) / 1000;
/// Retail HeightDieUpdate TargetHeight residual.
pub const SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT: f32 = 1.0;
/// Retail SpectreHowitzerShell GeometryMajorRadius residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS: f32 = 4.0;
/// Retail SpectreHowitzerShell Scale residual.
pub const SPECTRE_HOWITZER_SHELL_SCALE: f32 = 0.6;
/// Retail SpectreHowitzerShellLocomotor Speed residual (dist/sec; unused when
/// DumbProjectileBehavior is active, honesty residual for shell path).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SPEED: f32 = 1111.0;
/// Retail SpectreHowitzerShell PhysicsBehavior Mass residual.
pub const SPECTRE_HOWITZER_SHELL_MASS: f32 = 1.0;
/// Retail SpectreHowitzerShell GeometryHeight residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT: f32 = 4.0;
/// Retail SpectreHowitzerShell W3D model residual honesty.
pub const SPECTRE_HOWITZER_SHELL_MODEL: &str = "AVSpectreShell1";
/// Retail HeightDieUpdate OnlyWhenMovingDown residual (pad-safe loft).
pub const SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN: bool = true;
/// Retail InstantDeath DETONATED FX residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX: &str = "FX_NukeGLA";
/// Retail InstantDeath DETONATED DeathTypes residual (`NONE +DETONATED`).
pub const SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_TYPES: &str = "NONE +DETONATED";
/// Retail InstantDeath LASERED FX residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_LASERED_FX: &str = "FX_GenericMissileDisintegrate";
/// Retail InstantDeath LASERED DeathTypes residual (`NONE +LASERED`).
pub const SPECTRE_HOWITZER_SHELL_DEATH_LASERED_TYPES: &str = "NONE +LASERED";
/// Retail InstantDeath LASERED OCL residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_LASERED_OCL: &str = "OCL_GenericMissileDisintegrate";
/// Retail InstantDeath non-laser death FX residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX: &str = "FX_GenericMissileDeath";
/// Retail InstantDeath GENERIC DeathTypes residual (`ALL -LASERED -DETONATED`).
pub const SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_TYPES: &str = "ALL -LASERED -DETONATED";
/// Retail SpectreHowitzerShell ActiveBody MaxHealth residual.
pub const SPECTRE_HOWITZER_SHELL_MAX_HEALTH: f32 = 100.0;
/// Retail SpectreHowitzerShell GeometryIsSmall residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL: bool = true;
/// Retail SpectreHowitzerShell Shadow residual.
pub const SPECTRE_HOWITZER_SHELL_SHADOW: &str = "SHADOW_DECAL";
/// Retail SpectreHowitzerShell Geometry type residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY: &str = "Cylinder";
/// Retail SpectreHowitzerShell KindOf residual.
pub const SPECTRE_HOWITZER_SHELL_KIND_OF: &str = "PROJECTILE";
/// Retail SpectreHowitzerShell VisionRange residual.
pub const SPECTRE_HOWITZER_SHELL_VISION_RANGE: f32 = 0.0;
/// Retail SpectreHowitzerShell Armor residual.
pub const SPECTRE_HOWITZER_SHELL_ARMOR: &str = "ProjectileArmor";
/// Retail HeightDieUpdate TargetHeightIncludesStructures residual (No).
pub const SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_INCLUDES_STRUCTURES: bool = false;
/// Retail ActiveBody InitialHealth residual.
pub const SPECTRE_HOWITZER_SHELL_INITIAL_HEALTH: f32 = 100.0;
/// Retail DisplayName residual.
pub const SPECTRE_HOWITZER_SHELL_DISPLAY_NAME: &str = "OBJECT:Missile";
/// Retail EditorSorting residual.
pub const SPECTRE_HOWITZER_SHELL_EDITOR_SORTING: &str = "SYSTEM";
/// Retail W3DModelDraw OkToChangeModelColor residual.
pub const SPECTRE_HOWITZER_SHELL_OK_TO_CHANGE_MODEL_COLOR: bool = true;
/// Retail ArmorSet DamageFX residual (`None`).
pub const SPECTRE_HOWITZER_SHELL_DAMAGE_FX: &str = "None";
/// Retail SpectreHowitzerShellLocomotor template name residual
/// (commented out in Object when DumbProjectileBehavior is active; template still
/// exists for residual honesty).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_NAME: &str = "SpectreHowitzerShellLocomotor";
/// Retail SpectreHowitzerShellLocomotor Surfaces residual.
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SURFACES: &str = "AIR";
/// Retail SpectreHowitzerShellLocomotor Appearance residual.
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_APPEARANCE: &str = "THRUST";
/// Retail SpectreHowitzerShellLocomotor MinSpeed residual (dist/sec).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MIN_SPEED: f32 = 1111.0;
/// Retail SpectreHowitzerShellLocomotor Acceleration residual (dist/sec²).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ACCEL: f32 = 9160.0;
/// Retail SpectreHowitzerShellLocomotor TurnRate residual (degrees/sec).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_TURN_RATE: f32 = 99999.0;
/// Retail SpectreHowitzerShellLocomotor MaxThrustAngle residual (degrees).
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_MAX_THRUST_ANGLE: f32 = 90.0;
/// Retail SpectreHowitzerShellLocomotor Braking residual.
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_BRAKING: f32 = 0.0;
/// Retail SpectreHowitzerShellLocomotor AllowAirborneMotiveForce residual.
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_ALLOW_AIRBORNE: bool = true;
/// Retail SpectreHowitzerGun AcceptableAimDelta residual (degrees).
pub const SPECTRE_HOWITZER_ACCEPTABLE_AIM_DELTA: f32 = 180.0;
/// Retail SpectreHowitzerGun AttackRange residual.
pub const SPECTRE_HOWITZER_ATTACK_RANGE: f32 = 2222.0;
/// Retail SpectreHowitzerGun ProjectileCollidesWith residual.
pub const SPECTRE_HOWITZER_PROJECTILE_COLLIDES_WITH: &str = "STRUCTURES WALLS";
/// Retail SpectreHowitzerGun AntiGround residual.
pub const SPECTRE_HOWITZER_ANTI_GROUND: bool = true;
/// Retail SpectreHowitzerGun PrimaryDamage residual.
pub const SPECTRE_HOWITZER_PRIMARY_DAMAGE: f32 = 80.0;
/// Retail SpectreHowitzerGun DelayBetweenShots residual (msec).
///
/// Distinct from SpectreGunshipUpdate `HowitzerFiringRate` **300** ms used for
/// orbit residual cadence ([`SPECTRE_ORBIT_TICK_INTERVAL_FRAMES`]). Host combat
/// orbit still uses HowitzerFiringRate; this residual tracks the weapon template
/// field honesty only.
pub const SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_MS: u32 = 777;
/// Retail DelayBetweenShots 777 ms → frames @ 30 FPS.
pub const SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_FRAMES: u32 =
    (SPECTRE_HOWITZER_DELAY_BETWEEN_SHOTS_MS * 30) / 1000;
/// Retail SpectreHowitzerGun DamageType residual.
pub const SPECTRE_HOWITZER_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail SpectreHowitzerGun DeathType residual.
pub const SPECTRE_HOWITZER_DEATH_TYPE: &str = "EXPLODED";
/// Retail SpectreHowitzerGun RadiusDamageAffects residual.
pub const SPECTRE_HOWITZER_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Retail SpectreHowitzerGun ClipSize residual (0 == infinite).
pub const SPECTRE_HOWITZER_CLIP_SIZE: u32 = 0;
/// Retail SpectreHowitzerGun ClipReloadTime residual (msec).
pub const SPECTRE_HOWITZER_CLIP_RELOAD_TIME_MS: u32 = 0;
/// Retail SpectreHowitzerShellLocomotor GroupMovementPriority residual.
pub const SPECTRE_HOWITZER_SHELL_LOCOMOTOR_GROUP_PRIORITY: &str = "MOVES_BACK";
/// Retail SpectreHowitzerGun AntiAirborneVehicle residual.
pub const SPECTRE_HOWITZER_ANTI_AIRBORNE_VEHICLE: bool = false;
/// Retail SpectreHowitzerGun AntiAirborneInfantry residual.
pub const SPECTRE_HOWITZER_ANTI_AIRBORNE_INFANTRY: bool = false;
/// Retail SpectreHowitzerGun AntiSmallMissile residual.
pub const SPECTRE_HOWITZER_ANTI_SMALL_MISSILE: bool = false;
/// Retail SpectreHowitzerGun AntiBallisticMissile residual.
pub const SPECTRE_HOWITZER_ANTI_BALLISTIC_MISSILE: bool = false;
/// Retail SpectreHowitzerGun ProjectileObject residual.
pub const SPECTRE_HOWITZER_PROJECTILE_OBJECT: &str = "SpectreHowitzerShell";
/// Retail SpectreHowitzerGun ContinuousFireCoast residual (msec).
pub const SPECTRE_HOWITZER_CONTINUOUS_FIRE_COAST_MS: u32 = 2000;
/// Retail SpectreHowitzerGun VeterancyFireFX residual (HEROIC same tracer).
pub const SPECTRE_HOWITZER_VETERANCY_FIRE_FX: &str = "WeaponFX_GenericTankGunNoTracer";

/// SpectreHowitzerShell loft residual position after `frames` of pad-safe delay.
///
/// Retail: HeightDie InitialDelay 30f prevents pad detonation; host residual
/// drops shell from spawn height toward TargetHeight=1 with OnlyWhenMovingDown.
/// Fail-closed: not full DumbProjectileBehavior Object / live Physics flight.
#[inline]
pub fn howitzer_shell_loft_sample(
    spawn: Vec3,
    target: Vec3,
    frames: u32,
) -> (Vec3, bool /*moving_down*/, bool /*height_die*/) {
    let spawn_h = spawn.y.max(50.0); // residual loft from gun altitude honesty
    let speed = SPECTRE_HOWITZER_WEAPON_SPEED / SP_LOGIC_FPS; // ~33.3 /frame
    let mut pos = Vec3::new(spawn.x, spawn_h, spawn.z);
    let mut prev_y = pos.y;
    let mut moving_down = false;
    for f in 0..frames {
        let to = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
        let dist = (to.x * to.x + to.z * to.z).sqrt();
        if dist > f32::EPSILON {
            let advance = speed.min(dist);
            pos.x += (to.x / dist) * advance;
            pos.z += (to.z / dist) * advance;
        }
        // After InitialDelay, allow HeightDie sink residual.
        if f >= SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES {
            pos.y = (pos.y - speed * 0.5).max(SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT * 0.5);
        }
        moving_down = pos.y < prev_y;
        prev_y = pos.y;
    }
    let height_die = frames >= SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES
        && pos.y <= SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT
        && (moving_down || SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN);
    if height_die {
        pos.y = 0.0; // residual ground impact
    }
    (pos, moving_down, height_die)
}

/// C++ `SpectreGunshipUpdate::update` override clamp.
///
/// `constraintRadius = AttackAreaRadius - TargetingReticleRadius`.
/// `overrideTargetDelta = initial - dest`; if length > constraint,
/// `dest = initial - normalize(delta) * constraint`. Does not move `initial`.
#[inline]
pub fn clamp_spectre_override_destination(
    initial: Vec3,
    destination: Vec3,
    attack_area_radius: f32,
    targeting_reticle_radius: f32,
) -> Vec3 {
    let constraint = (attack_area_radius - targeting_reticle_radius).max(0.0);
    let dx = initial.x - destination.x;
    let dz = initial.z - destination.z;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist > constraint && dist > 1.0e-4 {
        let scale = constraint / dist;
        Vec3::new(
            initial.x - dx * scale,
            destination.y,
            initial.z - dz * scale,
        )
    } else {
        destination
    }
}

/// C++ `m_okToFireHowitzerCounter > m_howitzerFollowLag` (HowitzerFollowLag 12f).
#[inline]
pub fn spectre_howitzer_follow_ready(ok_to_fire_counter: u32) -> bool {
    ok_to_fire_counter > SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES
}

/// C++ `SpectreGunshipUpdate.cpp:609-623` StrafingIncrement wind + FollowLag counter.
///
/// Host ground plane is XZ. Dist < increment → snap to shoot-at and increment
/// the howitzer counter. Otherwise reset the counter and step toward shoot-at.
#[inline]
pub fn spectre_wind_gattling_aim(
    gattling_target: Vec3,
    position_to_shoot_at: Vec3,
    strafing_increment: f32,
    ok_to_fire_counter: u32,
) -> (Vec3, u32) {
    let dx = position_to_shoot_at.x - gattling_target.x;
    let dz = position_to_shoot_at.z - gattling_target.z;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist < strafing_increment {
        (position_to_shoot_at, ok_to_fire_counter.saturating_add(1))
    } else if dist > 1.0e-4 {
        let scale = strafing_increment / dist;
        (
            Vec3::new(
                gattling_target.x + dx * scale,
                position_to_shoot_at.y,
                gattling_target.z + dz * scale,
            ),
            0,
        )
    } else {
        (position_to_shoot_at, ok_to_fire_counter.saturating_add(1))
    }
}

/// C++ `SpectreGunshipUpdate::isFairDistanceFromShip` (cpp:713-731).
///
/// 2D ship-to-target (host XZ / C++ XY with Z=0) must exceed
/// `gunship_orbit_radius * 0.75`. Missing ship → false (skip acquire).
#[inline]
pub fn spectre_is_fair_distance_from_ship(
    ship_pos: Option<Vec3>,
    target_pos: Vec3,
    gunship_orbit_radius: f32,
) -> bool {
    let Some(ship) = ship_pos else {
        return false;
    };
    let dx = ship.x - target_pos.x;
    let dz = ship.z - target_pos.z;
    (dx * dx + dz * dz).sqrt() > gunship_orbit_radius * 0.75
}

/// Leftover `SpectreGunshipUpdate::is_disguised_as_enemy`.
/// KINDOF_DISGUISER + OBJECT_STATUS_DISGUISED + gunship relationship to the
/// disguise (apparent) team is ENEMIES.
#[inline]
pub fn spectre_orbit_is_disguised_as_enemy(
    is_disguiser: bool,
    disguised: bool,
    disguise_team_is_enemy: bool,
) -> bool {
    is_disguiser && disguised && disguise_team_is_enemy
}

/// Leftover `find_target_in_radius` stealth gate / C++
/// `PartitionFilterStealthedAndUndetected(false)`.
///
/// STEALTHED && !DETECTED blocks unless `is_disguised_as_enemy`.
/// `Object::is_effectively_stealthed` is wrong here: DISGUISED clears that
/// flag so any disguised unit would pass the gate.
#[inline]
pub fn spectre_orbit_stealthed_undetected_blocks(
    stealthed: bool,
    detected: bool,
    disguised_as_enemy: bool,
) -> bool {
    stealthed && !detected && !disguised_as_enemy
}

/// C++ `SpectreGunshipUpdate.cpp:498-507` acquire filters as a pure residual.
///
/// `PartitionFilterLiveMapEnemies` (alive + relationship ENEMIES — not
/// neutrals/allies) + `PartitionFilterStealthedAndUndetected(false)` +
/// `PartitionFilterPossibleToAttack` AntiAir=No (`SpectreHowitzerGun` /
/// `SpectreGattlingGun` AntiAirborne* **No**) + `PartitionFilterFreeOfFog`
/// (`ObjectShroudStatus::Clear` only).
#[inline]
pub fn spectre_orbit_target_passes_partition_filters(
    alive: bool,
    relationship_enemies: bool,
    stealthed_undetected: bool,
    is_air: bool,
    fog_clear: bool,
) -> bool {
    alive && relationship_enemies && !stealthed_undetected && !is_air && fog_clear
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_distance_skips_under_ship_and_missing_gunship() {
        let ship = Some(Vec3::new(0.0, 80.0, 0.0));
        let orbit = SPECTRE_GUNSHIP_ORBIT_RADIUS;
        // Directly under the ship (0 2D) — not fair.
        assert!(!spectre_is_fair_distance_from_ship(
            ship,
            Vec3::new(0.0, 0.0, 0.0),
            orbit
        ));
        // Just inside 0.75 * 250 = 187.5.
        assert!(!spectre_is_fair_distance_from_ship(
            ship,
            Vec3::new(180.0, 0.0, 0.0),
            orbit
        ));
        // Outside the 0.75 ring — gattling may acquire.
        assert!(spectre_is_fair_distance_from_ship(
            ship,
            Vec3::new(200.0, 0.0, 0.0),
            orbit
        ));
        // No live gunship position → fail-closed skip.
        assert!(!spectre_is_fair_distance_from_ship(
            None,
            Vec3::new(200.0, 0.0, 0.0),
            orbit
        ));
    }

    #[test]
    fn disguised_as_friend_is_stealthed_undetected_for_spectre() {
        // GLA Bomb Truck disguised as USA/ally: leftover is_disguised_as_enemy
        // is false, so STEALTHED && !DETECTED still blocks acquire.
        assert!(!spectre_orbit_is_disguised_as_enemy(true, true, false));
        assert!(spectre_orbit_stealthed_undetected_blocks(
            true, false, false
        ));
        assert!(!spectre_orbit_target_passes_partition_filters(
            true, true, true, false, true
        ));
    }

    #[test]
    fn disguised_as_enemy_exempts_stealth_gate() {
        // GLA Bomb Truck disguised as China vs USA Spectre: apparent team is
        // ENEMIES, so the stealth filter does not hide the truck. Real-team
        // ENEMIES then allows acquire.
        assert!(spectre_orbit_is_disguised_as_enemy(true, true, true));
        assert!(!spectre_orbit_stealthed_undetected_blocks(
            true, false, true
        ));
        assert!(spectre_orbit_target_passes_partition_filters(
            true, true, false, false, true
        ));
    }

    #[test]
    fn effectively_stealthed_would_wrongly_pass_any_disguise() {
        // is_effectively_stealthed = STEALTHED && !DETECTED && !DISGUISED.
        // Using that as the Spectre stealth bit lets every disguised unit
        // through (stealthed_undetected=false) and then real-team ENEMIES
        // shoots a friendly-presenting Bomb Truck.
        let effectively_stealthed = true && !false && !true;
        assert!(!effectively_stealthed);
        assert!(
            spectre_orbit_target_passes_partition_filters(
                true,
                true,
                effectively_stealthed,
                false,
                true
            ),
            "is_effectively_stealthed lets disguised-as-friend pass — leftover rejects"
        );
        assert!(
            !spectre_orbit_target_passes_partition_filters(
                true,
                true,
                spectre_orbit_stealthed_undetected_blocks(true, false, false),
                false,
                true
            ),
            "leftover stealth gate must skip disguised-as-friend"
        );
    }
}
