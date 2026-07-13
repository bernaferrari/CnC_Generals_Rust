//! Host special-power / superweapon strike residual.
//!
//! Residual slice: host `DoSpecialPower` for DaisyCutter / A10 / ScudStorm /
//! ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb /
//! ArtilleryBarrage / CruiseMissile queues a real strike that completes with
//! area damage on host GameLogic objects. NuclearMissile also spawns a residual
//! radiation field (`NukeRadiationFieldWeapon`) that ticks after impact.
//! AnthraxBomb also spawns a residual toxin field
//! (`AnthraxBombPoisonFieldWeapon` / `OCL_PoisonFieldAnthraxBomb`) that ticks
//! after impact. ScudStorm is a delayed multi-missile residual
//! (`ScudStormWeapon` ClipSize 9 / ScatterTarget + `ScudStormDamageWeapon`
//! primary/secondary blast) that also spawns residual LargePoisonField toxin
//! ticks after each missile. SpectreGunship completes orbit insertion with no
//! one-shot blast, then spawns a residual orbit field (`SpectreHowitzerGun`
//! residual) that periodically damages in `AttackAreaRadius` for science-tier
//! `OrbitTime` (L1/L2/L3). ParticleCannon (Particle Uplink) completes charge
//! residual with no one-shot blast, then spawns a residual continuous beam field
//! (`ParticleUplinkCannonUpdate` TotalFiringTime / TotalDamagePulses /
//! DamagePerSecond residual) that pulses damage at the target for the beam dwell.
//! CarpetBomb is a delayed multi-strike line residual (`SUPERWEAPON_CarpetBomb` /
//! `CarpetBombWeapon`): after bomber approach delay, applies explosive damage at
//! DropDelay-staggered epicenters along a line through the target with DropVariance
//! residual scatter (fail-closed vs full B52 OCL DeliverPayload transport Object).
//! ArtilleryBarrage is a delayed multi-shell scatter residual
//! (`SUPERWEAPON_ArtilleryBarrage1` / `ArtilleryBarrageDamageWeapon`): after
//! DelayDeliveryMax residual, applies explosive damage at WeaponErrorRadius-
//! scattered shell epicenters with per-shell DelayDelivery stagger (fail-closed
//! vs full ChinaArtilleryCannon OCL DeliverPayload transport Object).
//! CruiseMissile is a delayed loft-to-target residual
//! (`SUPR_SPECIAL_CRUISE_MISSILE` / `SupW_CruiseMissile` /
//! `SUPERWEAPON_CruiseMissile` / `MOABDetonationWeapon`): after NeutronMissile
//! loft residual delay, applies MOAB area damage at the target (fail-closed vs
//! full NeutronMissileUpdate door/loft path / OCL FireWeapon projectile /
//! MOABFlameWeapon secondary). Pending strikes (absolute `impact_frame`) are
//! captured in `WorldSnapshot.special_power_strikes` so mid-flight save/load
//! continues remaining delay and still fires impact / orbit / beam residual.
//!
//! Fail-closed: not full retail OCL / NeutronMissileUpdate flight / multi-blast
//! SlowDeath wave / multiplayer superweapon parity or C++ SpecialPowerModule
//! Xfer tables. Radiation / toxin / Spectre orbit / PUC beam residual is a
//! single host field (not full HazardousMaterialArmor / cleanup-hazard object
//! stack / SpectreGunshipUpdate continuous-fire ROF + ContinuousFireCoast residual
//! (host residual; SpectreHowitzerShell projectile residual closed at shell
//! spawn/FireFX/detonation/HeightDie InitialDelay + DumbProjectileBehavior /
//! Physics mass / InstantDeath path honesty — not full W3D shell drawable Object);
//! MODELCONDITION CONTINUOUS_FIRE_* honesty residual closed) /
//! ParticleUplinkCannonUpdate outer-node + connector laser residual closed at
//! STATUS_FIRING; intensity schedule residual closed
//! (CHARGING/PREPARING/ALMOST_READY/READY Light→Medium→Intense client residual);
//! OuterBeamWidth × width_scalar orbital laser draw / getCurrentLaserRadius
//! retail damage-radius formula honesty residual closed (host combat still caps
//! at PARTICLE_BEAM_RADIUS 50; retail peak = OuterBeamWidth×0.5×DamageRadiusScalar
//! = 44.2); manual beam driving residual closed (override destination +
//! ManualDrivingSpeed / ManualFastDrivingSpeed / DoubleClickToFastDriveDelay);
//! DamagePulseRemnant trail residual closed; swath sine residual closed;
//! WidthGrow damage-radius grow+hold+decay shrink residual closed;
//! TotalScorchMarks/RevealRange residual closed). CarpetBomb residual is
//! DropDelay-staggered multi-point blasts with DropVariance residual scatter
//! (not full AmericaJetB52 Object / pathfinder). ArtilleryBarrage residual is
//! WeaponErrorRadius-scattered shells with per-shell DelayDelivery stagger and
//! science-tier FormationSize (once-at-queue pure ADC GameLogicRandomValueReal residual
//! stored on the strike at queue; plan_due uses stored epicenters/shell frames —
//! fail-closed vs live mid-sim global stream mutation / full ChinaArtilleryCannon
//! transport Object). ScudStorm residual is ClipSize-9
//! ScatterTarget multi-missile + ScudStormDamageWeapon primary/secondary +
//! LargePoisonField residual, with Anthrax Beta/Gamma upgraded Secondary 200 +
//! upgraded poison 25 residual; PreAttack PER_CLIP + FireFX/IgnitionFX/launch
//! residual + Chem FXBone residual + ScudStormMissile MissileAIUpdate loft /
//! HeightDie / PreferredHeight spring residual closed (not full ThingFactory
//! projectile Object / live MissileAIUpdate physics flight sim).
//! OuterBeamWidth multi-beam NumBeams + ScrollRate / TilingScalar residual closed
//! (host tracks multi-beam cylinder count + scroll UV honesty).
//! Multi-beam soft-edge width/alpha/color lerp residual closed
//! (W3DLaserDraw scale = i/(NumBeams-1) cylinders + tile-factor honesty;
//! fail-closed vs full SegLineRenderer GPU texture atlas submit).
//! ScudStormMissile ballistic flight residual closed
//! (locomotor speed/accel + OnlyWhenMovingDown/SnapToGround + model UBScudStrm_M
//! + geometry residual; fail-closed vs full ThingFactory Object physics).
//! SpectreHowitzerShell W3D ModelDraw residual closed
//! (model AVSpectreShell1 + Scale/Shadow/MaxHealth honesty; fail-closed vs full
//! W3D drawable Object / live Physics flight).
//! Outer-node bone layout residual closed (FX01..FX05 ring positions host residual;
//! fail-closed vs full W3D bone-world extract / LaserUpdate drawable matrix).
//! CruiseMissile residual is a MOAB primary + MOABFlame secondary residual
//! (not full loft projectile / HeightDieUpdate / door animation / tree burn state).

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const SP_LOGIC_FPS: f32 = 30.0;

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

// --- Anthrax toxin residual (retail AnthraxBombPoisonFieldWeapon / LifetimeUpdate) ---

/// Retail `AnthraxBombPoisonFieldWeapon` PrimaryDamage.
pub const ANTHRAX_TOXIN_DAMAGE_PER_TICK: f32 = 40.0;
/// Retail `AnthraxBombPoisonFieldWeapon` PrimaryDamageRadius.
pub const ANTHRAX_TOXIN_RADIUS: f32 = 300.0;
/// Retail DelayBetweenShots = 500 ms → 15 frames @ 30 FPS.
pub const ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES: u32 = 15;
/// Retail PoisonFieldAnthraxBomb LifetimeUpdate Min/MaxLifetime = 60000 ms @ 30 FPS.
pub const ANTHRAX_TOXIN_DURATION_FRAMES: u32 = 1800;
/// Residual ambient cue for the anthrax pool (`PoisonFieldAnthraxBomb.SoundAmbient`).
pub const ANTHRAX_TOXIN_AUDIO: &str = "AnthraxPoolAmbientLoop";

// --- Spectre Gunship orbit residual (retail SpectreHowitzerGun / OrbitTime) ---

/// Retail `SpectreHowitzerGun` PrimaryDamage (orbit residual tick).
/// Fail-closed vs full gattling-strafe + howitzer projectile + random offset.
pub const SPECTRE_ORBIT_DAMAGE_PER_TICK: f32 = 80.0;
/// Retail `SpectreGunshipUpdate` AttackAreaRadius / RadiusCursorRadius.
pub const SPECTRE_ORBIT_RADIUS: f32 = 200.0;
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
/// Retail InstantDeath LASERED FX residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_LASERED_FX: &str = "FX_GenericMissileDisintegrate";
/// Retail InstantDeath non-laser death FX residual honesty.
pub const SPECTRE_HOWITZER_SHELL_DEATH_GENERIC_FX: &str = "FX_GenericMissileDeath";
/// Retail SpectreHowitzerShell ActiveBody MaxHealth residual.
pub const SPECTRE_HOWITZER_SHELL_MAX_HEALTH: f32 = 100.0;
/// Retail SpectreHowitzerShell GeometryIsSmall residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL: bool = true;
/// Retail SpectreHowitzerShell Shadow residual.
pub const SPECTRE_HOWITZER_SHELL_SHADOW: &str = "SHADOW_DECAL";
/// Retail SpectreHowitzerShell Geometry type residual.
pub const SPECTRE_HOWITZER_SHELL_GEOMETRY: &str = "Cylinder";

// --- Particle Uplink continuous beam residual (ParticleUplinkCannonUpdate) ---

/// Retail `ParticleUplinkCannonUpdate` TotalFiringTime = 3500 ms → 105 frames @ 30 FPS.
pub const PARTICLE_BEAM_DURATION_FRAMES: u32 = 105;
/// Retail TotalDamagePulses = 40.
pub const PARTICLE_BEAM_TOTAL_PULSES: u32 = 40;
/// Retail DamagePerSecond = 400.
/// damagePerPulse = (TotalFiringFrames/FPS * DamagePerSecond) / TotalDamagePulses
///                 = (105/30 * 400) / 40 = 35.
pub const PARTICLE_BEAM_DAMAGE_PER_PULSE: f32 = 35.0;
/// Residual pulse interval floor: TotalFiringTime / TotalDamagePulses → 105/40
/// ≈ 2.625 frames. Host residual prefers fractional nextFactor scheduling
/// ([`particle_next_pulse_frame`]); this constant remains the minimum gap honesty.
pub const PARTICLE_BEAM_TICK_INTERVAL_FRAMES: u32 = 3;
/// Residual damage radius at target (fail-closed vs laser radius ×
/// DamageRadiusScalar grow/shrink matrix; retail scalar 3.4 on dynamic beam).
pub const PARTICLE_BEAM_RADIUS: f32 = 50.0;
/// Retail `ParticleUplinkCannonUpdate` DamageRadiusScalar = 3.4 (honesty residual).
/// Host damage radius is a fixed residual radius; scalar documents retail ratio.
pub const PARTICLE_DAMAGE_RADIUS_SCALAR: f32 = 3.4;
/// Retail SwathOfDeathDistance — beam epicenter walks this total distance over
/// TotalFiringTime (S-curve residual).
pub const PARTICLE_SWATH_OF_DEATH_DISTANCE: f32 = 200.0;
/// Retail SwathOfDeathAmplitude — lateral sine amplitude of swath residual.
pub const PARTICLE_SWATH_OF_DEATH_AMPLITUDE: f32 = 50.0;
/// Retail WidthGrowTime = 2000 ms → 60 frames @ 30 FPS.
/// Laser radius ramps 0→full over this window at orbital birth, and shrinks
/// full→0 over the same window after TotalFiringTime (`LaserUpdate::setDecayFrames`).
pub const PARTICLE_WIDTH_GROW_FRAMES: u32 = (2000 * 30) / 1000;
/// Full orbital beam lifetime residual: TotalFiringTime + WidthGrowTime decay tail.
/// C++: `orbitalDeathFrame = orbitalDecayStart + widthGrowFrames` where
/// `orbitalDecayStart - orbitalBirth = totalFiringFrames`.
pub const PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES: u32 =
    PARTICLE_BEAM_DURATION_FRAMES + PARTICLE_WIDTH_GROW_FRAMES;
/// Retail OuterBeamWidth residual for OrbitalLaser honesty (26.0).
///
/// Retail damage radius formula (`LaserUpdate::getCurrentLaserRadius` ×
/// `DamageRadiusScalar`):
/// `getLaserTemplateWidth() = OuterBeamWidth * 0.5` → peak laser r = **13.0**,
/// peak damage = 13 × 3.4 = **44.2**. Host combat residual still caps at
/// [`PARTICLE_BEAM_RADIUS`] (**50**) for fail-closed parity with prior host tests;
/// OuterBeamWidth draw / retail-formula honesty is tracked separately.
pub const PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH: f32 = 26.0;
/// Retail InnerBeamWidth residual for OrbitalLaser W3DLaserDraw.
pub const PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH: f32 = 0.6;
/// Retail NumBeams residual (overlapping cylinders).
pub const PARTICLE_ORBITAL_LASER_NUM_BEAMS: u32 = 12;
/// Retail ScrollRate residual (toward muzzle negative).
pub const PARTICLE_ORBITAL_LASER_SCROLL_RATE: f32 = -1.75;
/// Retail TilingScalar residual.
pub const PARTICLE_ORBITAL_LASER_TILING_SCALAR: f32 = 0.15;
/// Retail W3DLaserDraw Texture residual.
pub const PARTICLE_ORBITAL_LASER_TEXTURE: &str = "EXNoise02.tga";
/// Retail OrbitalLaser InnerColor residual (R:255 G:255 B:255 A:250).
pub const PARTICLE_ORBITAL_LASER_INNER_COLOR: (f32, f32, f32, f32) =
    (1.0, 1.0, 1.0, 250.0 / 255.0);
/// Retail OrbitalLaser OuterColor residual (R:0 G:0 B:255 A:150).
pub const PARTICLE_ORBITAL_LASER_OUTER_COLOR: (f32, f32, f32, f32) =
    (0.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail OrbitalLaser Tile residual (`Tile = Yes`).
pub const PARTICLE_ORBITAL_LASER_TILE: bool = true;
/// Host residual texture aspect for tile-factor honesty (fail-closed vs live surface desc).
pub const PARTICLE_ORBITAL_LASER_TEXTURE_ASPECT: f32 = 1.0;
/// Retail Medium connector laser OuterBeamWidth residual.
pub const PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH: f32 = 1.2;
/// Retail Intense connector laser OuterBeamWidth residual.
pub const PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH: f32 = 2.0;
/// Retail Medium connector NumBeams residual.
pub const PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS: u32 = 4;
/// Retail Intense connector NumBeams residual.
pub const PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS: u32 = 5;
/// Retail connector laser Texture residual.
pub const PARTICLE_CONNECTOR_LASER_TEXTURE: &str = "EXLaser.tga";
/// Retail RevealRange = 50 — gratuitous vision at each scorch/GroundHitFX site.
pub const PARTICLE_REVEAL_RANGE: f32 = 50.0;
/// Retail TotalScorchMarks = 20 (also gates GroundHitFX / reveal cadence).
pub const PARTICLE_TOTAL_SCORCH_MARKS: u32 = 20;
/// Retail ScorchMarkScalar = 2.4 (scorch radius = laser radius × scalar).
pub const PARTICLE_SCORCH_MARK_SCALAR: f32 = 2.4;
/// Residual GroundHitFX name honesty (TotalScorchMarks determines call count).
pub const PARTICLE_GROUND_HIT_FX: &str = "FX_ParticleUplinkCannon_BeamHitsGround";
/// Retail ManualDrivingSpeed = 20 (world units per second).
/// Host residual converts to per-frame via [`particle_manual_speed_per_frame`].
pub const PARTICLE_MANUAL_DRIVING_SPEED: f32 = 20.0;
/// Retail ManualFastDrivingSpeed = 40 (world units per second; double-click).
pub const PARTICLE_MANUAL_FAST_DRIVING_SPEED: f32 = 40.0;
/// Retail DoubleClickToFastDriveDelay = 500 ms → 15 frames.
pub const PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES: u32 = (500 * 30) / 1000;
/// Residual ambient cue while beam is annihilating ground.
pub const PARTICLE_BEAM_AUDIO: &str = "ParticleUplinkCannon_GroundAnnihilationSoundLoop";
/// Retail OuterEffectNumBones = 5 (outer node FX bones / connector lasers).
pub const PARTICLE_OUTER_EFFECT_NUM_BONES: u32 = 5;
/// Retail OuterEffectBoneName base (`FX01`..`FX05` layout residual).
pub const PARTICLE_OUTER_EFFECT_BONE_NAME: &str = "FX";
/// Retail ConnectorBoneName.
pub const PARTICLE_CONNECTOR_BONE_NAME: &str = "FXConnector";
/// Retail FireBoneName (main beam origin).
pub const PARTICLE_FIRE_BONE_NAME: &str = "FXMain";
/// Host residual outer-node ring radius (fail-closed vs live W3D bone-world convert).
///
/// Retail bones sit on the PUC dish rim; host residual places FX01..FX05 on a
/// unit circle of this radius around the building residual origin.
pub const PARTICLE_OUTER_NODE_RING_RADIUS: f32 = 40.0;
/// Host residual outer-node height above building origin (dish FX height residual).
pub const PARTICLE_OUTER_NODE_RING_HEIGHT: f32 = 25.0;
/// Retail OuterNodesLightFlareParticleSystem.
pub const PARTICLE_OUTER_NODE_LIGHT_FLARE: &str = "ParticleUplinkCannon_OuterNodeLightFlare";
/// Retail OuterNodesMediumFlareParticleSystem.
pub const PARTICLE_OUTER_NODE_MEDIUM_FLARE: &str = "ParticleUplinkCannon_OuterNodeMediumFlare";
/// Retail OuterNodesIntenseFlareParticleSystem (STATUS_FIRING residual).
pub const PARTICLE_OUTER_NODE_INTENSE_FLARE: &str = "ParticleUplinkCannon_OuterNodeIntenseFlare";
/// Retail ConnectorMediumLaserName.
pub const PARTICLE_CONNECTOR_MEDIUM_LASER: &str = "ParticleUplinkCannon_MediumConnectorLaser";
/// Retail ConnectorIntenseLaserName (STATUS_FIRING residual).
pub const PARTICLE_CONNECTOR_INTENSE_LASER: &str = "ParticleUplinkCannon_IntenseConnectorLaser";
/// Retail LaserBaseLightFlareParticleSystemName (ready residual honesty).
pub const PARTICLE_LASER_BASE_READY_FLARE: &str = "ParticleUplinkCannon_LaserBaseReadyToFire";
/// Retail ParticleBeamLaserName (ground↔orbit + orbit→target lasers).
pub const PARTICLE_ORBITAL_LASER_NAME: &str = "ParticleUplinkCannon_OrbitalLaser";
/// Retail BeginChargeTime = 5000 ms → 150 frames @ 30 FPS.
/// Outer nodes begin Light flare residual before ready-to-fire.
pub const PARTICLE_BEGIN_CHARGE_FRAMES: u32 = (5000 * 30) / 1000;
/// Retail RaiseAntennaTime = 4667 ms → 140 frames @ 30 FPS.
/// Hatch opens / antenna raises (MODELCONDITION_UNPACKING residual).
pub const PARTICLE_RAISE_ANTENNA_FRAMES: u32 = (4667 * 30) / 1000;
/// Retail ReadyDelayTime = 2000 ms → 60 frames @ 30 FPS.
/// Antenna raised → ready-to-fire (MODELCONDITION_DEPLOYED residual).
pub const PARTICLE_READY_DELAY_FRAMES: u32 = (2000 * 30) / 1000;
/// Retail BeamTravelTime = 2500 ms → 75 frames @ 30 FPS.
/// Dish→ground travel residual (host impact_delay is a charge+travel subset).
pub const PARTICLE_BEAM_TRAVEL_FRAMES: u32 = (2500 * 30) / 1000;
/// Retail DelayBetweenLaunchFX = 1000 ms → 30 frames @ 30 FPS.
pub const PARTICLE_LAUNCH_FX_INTERVAL_FRAMES: u32 = (1000 * 30) / 1000;
/// Retail BeamLaunchFX residual (refreshed while STATUS_FIRING).
pub const PARTICLE_BEAM_LAUNCH_FX: &str = "FX_ParticleUplinkCannon_BeamLaunchIteration";
/// Retail PoweringUpSoundLoop (STATUS_CHARGING residual honesty).
pub const PARTICLE_POWERUP_AUDIO: &str = "ParticleUplinkCannon_PowerupSoundLoop";
/// Retail UnpackToIdleSoundLoop (STATUS_PREPARING residual honesty).
pub const PARTICLE_UNPACK_AUDIO: &str = "ParticleUplinkCannon_UnpackToIdleSoundLoop";
/// Retail FiringToPackSoundLoop (STATUS_FIRING residual honesty).
pub const PARTICLE_FIRING_TO_PACK_AUDIO: &str = "ParticleUplinkCannon_FiringToPackSoundLoop";

/// Retail `ParticleUplinkCannonUpdate` logical / client status residual.
///
/// C++ `PUCStatus` order is load-bearing for honesty comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum ParticleUplinkStatus {
    #[default]
    Idle = 0,
    Charging = 1,
    Preparing = 2,
    AlmostReady = 3,
    ReadyToFire = 4,
    Prefire = 5,
    Firing = 6,
    Postfire = 7,
    Packing = 8,
}

impl ParticleUplinkStatus {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Charging,
            2 => Self::Preparing,
            3 => Self::AlmostReady,
            4 => Self::ReadyToFire,
            5 => Self::Prefire,
            6 => Self::Firing,
            7 => Self::Postfire,
            8 => Self::Packing,
            _ => Self::Idle,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Charging => "CHARGING",
            Self::Preparing => "PREPARING",
            Self::AlmostReady => "ALMOST_READY",
            Self::ReadyToFire => "READY_TO_FIRE",
            Self::Prefire => "PREFIRE",
            Self::Firing => "FIRING",
            Self::Postfire => "POSTFIRE",
            Self::Packing => "PACKING",
        }
    }
}

/// Retail `IntensityTypes` residual for outer-node / connector / laser-base FX.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum ParticleIntensity {
    #[default]
    None = 0,
    Light = 1,
    Medium = 2,
    Intense = 3,
}

impl ParticleIntensity {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Light,
            2 => Self::Medium,
            3 => Self::Intense,
            _ => Self::None,
        }
    }

    pub fn outer_flare_name(self) -> &'static str {
        match self {
            Self::Light => PARTICLE_OUTER_NODE_LIGHT_FLARE,
            Self::Medium => PARTICLE_OUTER_NODE_MEDIUM_FLARE,
            Self::Intense => PARTICLE_OUTER_NODE_INTENSE_FLARE,
            Self::None => "",
        }
    }

    pub fn connector_laser_name(self) -> &'static str {
        match self {
            Self::Medium => PARTICLE_CONNECTOR_MEDIUM_LASER,
            Self::Intense => PARTICLE_CONNECTOR_INTENSE_LASER,
            // Retail has no Light connector laser; empty honesty residual.
            _ => "",
        }
    }
}

/// Host-testable client-effects residual for one `setClientStatus` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleClientEffects {
    pub outer_nodes: u32,
    pub outer_intensity: ParticleIntensity,
    pub connector_lasers: u32,
    pub connector_intensity: ParticleIntensity,
    pub connector_flare: u32,
    pub laser_base: u32,
    pub laser_base_intensity: ParticleIntensity,
    pub ground_to_orbit: u32,
}

impl ParticleClientEffects {
    pub const EMPTY: Self = Self {
        outer_nodes: 0,
        outer_intensity: ParticleIntensity::None,
        connector_lasers: 0,
        connector_intensity: ParticleIntensity::None,
        connector_flare: 0,
        laser_base: 0,
        laser_base_intensity: ParticleIntensity::None,
        ground_to_orbit: 0,
    };
}

/// Retail `setClientStatus` residual schedule (`ParticleUplinkCannonUpdate.cpp`).
///
/// Fail-closed: not full bone-world convert / LaserUpdate drawable objects /
/// shroud client removeAllEffects path.
pub fn particle_client_effects_for_status(status: ParticleUplinkStatus) -> ParticleClientEffects {
    match status {
        ParticleUplinkStatus::Charging => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Light,
            ..ParticleClientEffects::EMPTY
        },
        ParticleUplinkStatus::Preparing => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Medium,
            ..ParticleClientEffects::EMPTY
        },
        ParticleUplinkStatus::AlmostReady => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Medium,
            connector_lasers: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_intensity: ParticleIntensity::Medium,
            connector_flare: 1,
            ..ParticleClientEffects::EMPTY
        },
        ParticleUplinkStatus::ReadyToFire => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Medium,
            connector_lasers: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_intensity: ParticleIntensity::Medium,
            connector_flare: 1,
            laser_base: 1,
            laser_base_intensity: ParticleIntensity::Light,
            ..ParticleClientEffects::EMPTY
        },
        ParticleUplinkStatus::Firing => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Intense,
            connector_lasers: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_intensity: ParticleIntensity::Intense,
            connector_flare: 1,
            laser_base: 1,
            laser_base_intensity: ParticleIntensity::Intense,
            ground_to_orbit: 1,
        },
        ParticleUplinkStatus::Postfire => ParticleClientEffects {
            outer_nodes: PARTICLE_OUTER_EFFECT_NUM_BONES,
            outer_intensity: ParticleIntensity::Medium,
            connector_lasers: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_intensity: ParticleIntensity::Medium,
            connector_flare: 1,
            laser_base: 1,
            laser_base_intensity: ParticleIntensity::Medium,
            ground_to_orbit: 1,
        },
        ParticleUplinkStatus::Idle
        | ParticleUplinkStatus::Prefire
        | ParticleUplinkStatus::Packing => ParticleClientEffects::EMPTY,
    }
}

/// Pre-attack status residual from special-power ready countdown.
///
/// C++ (not currently attacking):
/// - `readyToFireFrame <= now` → READY_TO_FIRE
/// - `almostReadyFrame <= now` → ALMOST_READY
/// - `raiseAntennaFrame <= now` → PREPARING
/// - `beginChargeFrame <= now` → CHARGING
/// - else IDLE
///
/// Host residual anchors `ready_to_fire_frame` at the ParticleCannon impact
/// frame (beam spawn / orbital birth residual).
pub fn particle_status_for_ready_countdown(
    now: u32,
    ready_to_fire_frame: u32,
) -> ParticleUplinkStatus {
    if now >= ready_to_fire_frame {
        return ParticleUplinkStatus::ReadyToFire;
    }
    let almost_ready = ready_to_fire_frame.saturating_sub(PARTICLE_READY_DELAY_FRAMES);
    if now >= almost_ready {
        return ParticleUplinkStatus::AlmostReady;
    }
    let raise_antenna = almost_ready.saturating_sub(PARTICLE_RAISE_ANTENNA_FRAMES);
    if now >= raise_antenna {
        return ParticleUplinkStatus::Preparing;
    }
    let begin_charge = raise_antenna.saturating_sub(PARTICLE_BEGIN_CHARGE_FRAMES);
    if now >= begin_charge {
        return ParticleUplinkStatus::Charging;
    }
    ParticleUplinkStatus::Idle
}

/// Attack-phase status residual after `initiateIntentToDoSpecialPower`.
///
/// C++ (startAttack set):
/// - `endDecayFrame <= now` → PACKING
/// - `startDecayFrame <= now` → POSTFIRE
/// - else → FIRING
pub fn particle_status_for_attack(
    now: u32,
    start_attack_frame: u32,
    total_firing_frames: u32,
    width_grow_frames: u32,
) -> ParticleUplinkStatus {
    let start_decay = start_attack_frame.saturating_add(total_firing_frames);
    let end_decay = start_decay.saturating_add(width_grow_frames);
    if now >= end_decay {
        ParticleUplinkStatus::Packing
    } else if now >= start_decay {
        ParticleUplinkStatus::Postfire
    } else if now >= start_attack_frame {
        ParticleUplinkStatus::Firing
    } else {
        ParticleUplinkStatus::ReadyToFire
    }
}

/// Apply pre-fire intensity schedule residual onto a ParticleCannon strike.
///
/// Anchors ready-to-fire at `strike.impact_frame` (host beam spawn residual).
fn apply_particle_charge_status(strike: &mut HostSpecialPowerStrike, now: u32) {
    if strike.kind != HostSuperweaponKind::ParticleCannon {
        return;
    }
    let next = particle_status_for_ready_countdown(now, strike.impact_frame);
    if next == strike.particle_status {
        return;
    }
    strike.particle_status = next;
    strike.particle_intensity_transitions =
        strike.particle_intensity_transitions.saturating_add(1);
    if next.as_u8() > strike.particle_status_peak.as_u8() {
        strike.particle_status_peak = next;
    }
    match next {
        ParticleUplinkStatus::Charging => {
            strike.particle_charging_applications =
                strike.particle_charging_applications.saturating_add(1);
        }
        ParticleUplinkStatus::Preparing => {
            strike.particle_preparing_applications =
                strike.particle_preparing_applications.saturating_add(1);
            strike.particle_model_unpacking_sets =
                strike.particle_model_unpacking_sets.saturating_add(1);
        }
        ParticleUplinkStatus::AlmostReady => {
            strike.particle_almost_ready_applications =
                strike.particle_almost_ready_applications.saturating_add(1);
            strike.particle_model_deployed_sets =
                strike.particle_model_deployed_sets.saturating_add(1);
        }
        ParticleUplinkStatus::ReadyToFire => {
            strike.particle_ready_applications =
                strike.particle_ready_applications.saturating_add(1);
            strike.particle_model_deployed_sets =
                strike.particle_model_deployed_sets.saturating_add(1);
        }
        ParticleUplinkStatus::Packing => {
            strike.particle_model_packing_sets =
                strike.particle_model_packing_sets.saturating_add(1);
        }
        _ => {}
    }
}

/// Manual drive speed per logic frame residual.
///
/// C++: `speed /= LOGICFRAMES_PER_SECOND` after selecting ManualDrivingSpeed or
/// ManualFastDrivingSpeed.
pub fn particle_manual_speed_per_frame(fast: bool) -> f32 {
    let speed = if fast {
        PARTICLE_MANUAL_FAST_DRIVING_SPEED
    } else {
        PARTICLE_MANUAL_DRIVING_SPEED
    };
    speed / SP_LOGIC_FPS
}

/// True when double-click gap is within [`PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES`].
///
/// C++: `m_lastDrivingClickFrame - m_2ndLastDrivingClickFrame < delay`.
pub fn particle_is_fast_drive(last_click_frame: u32, second_last_click_frame: u32) -> bool {
    last_click_frame.saturating_sub(second_last_click_frame)
        < PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES
}

/// Next absolute frame for the next Particle Uplink damage pulse (fractional residual).
///
/// C++ after each pulse: `nextFactor = damagePulsesMade / totalDamagePulses`,
/// `m_nextDamagePulseFrame = orbitalBirth + nextFactor * orbitalLifetime`.
/// Host residual uses the same nextFactor schedule (fail-closed vs full laser
/// grow/shrink PossibleNextShot timing).
pub fn particle_next_pulse_frame(spawn_frame: u32, pulses_made: u32) -> u32 {
    if PARTICLE_BEAM_TOTAL_PULSES == 0 {
        return spawn_frame.saturating_add(PARTICLE_BEAM_TICK_INTERVAL_FRAMES);
    }
    let factor = (pulses_made as f32) / (PARTICLE_BEAM_TOTAL_PULSES as f32);
    let offset = (factor * (PARTICLE_BEAM_DURATION_FRAMES as f32)).floor() as u32;
    let next = spawn_frame.saturating_add(offset);
    // Ensure strictly forward progress of at least 1 frame when pulses remain.
    next.max(spawn_frame.saturating_add(1))
}

/// Residual SwathOfDeath epicenter offset for a damage pulse.
///
/// C++ ParticleUplinkCannonUpdate (non-manual mode):
/// `factor = (now - orbitalBirth) / orbitalLifetime`,
/// `radians = factor * TWO_PI - PI`,
/// `cxDistance = factor * SwathOfDeathDistance - SwathOfDeathDistance/2`,
/// `cxHeight = sin(radians) * SwathOfDeathAmplitude`,
/// then rotate onto building→target axis.
///
/// Host residual uses pulse index as time factor and applies offset in host
/// x/z plane relative to the click epicenter (fail-closed vs full building
/// orientation rotation matrix / terrain Z).
pub fn particle_swath_offset(pulses_made_before_this_pulse: u32) -> Vec3 {
    let factor = if PARTICLE_BEAM_TOTAL_PULSES == 0 {
        0.0
    } else {
        (pulses_made_before_this_pulse as f32) / (PARTICLE_BEAM_TOTAL_PULSES as f32)
    };
    let factor = factor.clamp(0.0, 1.0);
    let radians = (factor * std::f32::consts::TAU) - std::f32::consts::PI;
    let cx_distance =
        (factor * PARTICLE_SWATH_OF_DEATH_DISTANCE) - (PARTICLE_SWATH_OF_DEATH_DISTANCE * 0.5);
    let cx_height = radians.sin() * PARTICLE_SWATH_OF_DEATH_AMPLITUDE;
    // Host gameplay plane: C++ x → host x, C++ y → host z.
    Vec3::new(cx_distance, 0.0, cx_height)
}

/// Absolute residual damage epicenter for a pulse at field spawn position.
pub fn particle_swath_epicenter(base: Vec3, pulses_made_before_this_pulse: u32) -> Vec3 {
    base + particle_swath_offset(pulses_made_before_this_pulse)
}

/// Absolute frame when WidthGrow decay starts (`LaserUpdate::setDecayFrames`).
///
/// Retail: `orbitalDecayStart = startAttack + totalFiring + beamTravel` relative
/// to orbital birth → `spawn + TotalFiringTime`.
pub fn particle_decay_start_frame(spawn_frame: u32) -> u32 {
    spawn_frame.saturating_add(PARTICLE_BEAM_DURATION_FRAMES)
}

/// Absolute frame when the orbital laser dies after decay shrink.
///
/// Retail: `orbitalDeathFrame = orbitalDecayStart + widthGrowFrames`.
pub fn particle_death_frame(spawn_frame: u32) -> u32 {
    spawn_frame.saturating_add(PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES)
}

/// Laser width scalar residual (`LaserUpdate::m_currentWidthScalar`).
///
/// Retail lifecycle relative to orbital birth (`spawn_frame`):
/// - **Grow** `[spawn, spawn+WidthGrow]`: `scalar = elapsed / WidthGrowTime` (0→1)
/// - **Hold** `(spawn+WidthGrow, spawn+TotalFiring]`: scalar = 1.0
/// - **Decay** `(spawn+TotalFiring, spawn+TotalFiring+WidthGrow]`:
///   `scalar = 1 - (now - decayStart) / WidthGrowTime` (1→0)
/// - **Dead** after orbital death: 0.0
///
/// Fail-closed: not full OuterBeamWidth × scalar GPU laser / client drawable.
pub fn particle_width_scalar(spawn_frame: u32, current_frame: u32) -> f32 {
    if PARTICLE_WIDTH_GROW_FRAMES == 0 {
        return 1.0;
    }
    if current_frame <= spawn_frame {
        return 0.0;
    }
    let grow_end = spawn_frame.saturating_add(PARTICLE_WIDTH_GROW_FRAMES);
    let decay_start = particle_decay_start_frame(spawn_frame);
    let death = particle_death_frame(spawn_frame);

    if current_frame <= grow_end {
        let elapsed = current_frame.saturating_sub(spawn_frame) as f32;
        return (elapsed / (PARTICLE_WIDTH_GROW_FRAMES as f32)).clamp(0.0, 1.0);
    }
    // Hold full width through TotalFiringTime (inclusive of decay_start frame —
    // C++ setDecayFrames initializes scalar to 1.0 on the decay-start frame).
    if current_frame <= decay_start {
        return 1.0;
    }
    if current_frame >= death {
        return 0.0;
    }
    let elapsed = current_frame.saturating_sub(decay_start) as f32;
    (1.0 - elapsed / (PARTICLE_WIDTH_GROW_FRAMES as f32)).clamp(0.0, 1.0)
}

/// Residual damage radius at `current_frame` under WidthGrow grow/hold/decay.
///
/// Full radius is [`PARTICLE_BEAM_RADIUS`] while hold. Early grow and late decay
/// pulses use a smaller radius (retail laser radius × width scalar matrix).
pub fn particle_beam_damage_radius(spawn_frame: u32, current_frame: u32) -> f32 {
    PARTICLE_BEAM_RADIUS * particle_width_scalar(spawn_frame, current_frame)
}

/// Retail `W3DLaserDraw::getLaserTemplateWidth()` residual (`OuterBeamWidth * 0.5`).
#[inline]
pub fn particle_orbital_laser_template_width() -> f32 {
    PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH * 0.5
}

/// Retail `LaserUpdate::getCurrentLaserRadius()` residual.
///
/// `getLaserTemplateWidth() * m_currentWidthScalar` (OuterBeamWidth/2 × scalar).
#[inline]
pub fn particle_orbital_laser_current_radius(spawn_frame: u32, current_frame: u32) -> f32 {
    particle_orbital_laser_template_width() * particle_width_scalar(spawn_frame, current_frame)
}

/// Retail visual OuterBeamWidth × width_scalar residual (W3DLaserDraw cylinder width).
///
/// Fail-closed: not full GPU multi-beam soft edge / texture atlas submit
/// (NumBeams + ScrollRate residual tracked separately).
#[inline]
pub fn particle_orbital_laser_draw_width(spawn_frame: u32, current_frame: u32) -> f32 {
    PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH * particle_width_scalar(spawn_frame, current_frame)
}

/// Retail W3DLaserDraw multi-beam cylinder count residual (`NumBeams`).
#[inline]
pub fn particle_orbital_laser_num_beams() -> u32 {
    PARTICLE_ORBITAL_LASER_NUM_BEAMS
}

/// Retail W3DLaserDraw texture scroll UV residual (`ScrollRate` × elapsed seconds).
///
/// C++ accumulates `m_textureScrollRate * dt` each client draw; host residual
/// samples elapsed logic frames as seconds (`frames / SP_LOGIC_FPS`).
/// Negative ScrollRate scrolls toward muzzle.
#[inline]
pub fn particle_orbital_laser_scroll_uv(spawn_frame: u32, current_frame: u32) -> f32 {
    if current_frame <= spawn_frame {
        return 0.0;
    }
    let elapsed_sec = (current_frame - spawn_frame) as f32 / SP_LOGIC_FPS;
    PARTICLE_ORBITAL_LASER_SCROLL_RATE * elapsed_sec
}

/// Retail W3DLaserDraw tiling residual (`TilingScalar` honesty).
///
/// Full UV packing uses segment length / beam width × aspect × TilingScalar;
/// host residual exposes the scalar constant for multi-beam honesty.
#[inline]
pub fn particle_orbital_laser_tiling_scalar() -> f32 {
    PARTICLE_ORBITAL_LASER_TILING_SCALAR
}

/// Soft-edge scale residual for multi-beam cylinder index `i` (`0..NumBeams-1`).
///
/// C++ W3DLaserDraw: `scale = i / (m_numBeams - 1.0f)` when NumBeams > 1.
/// Scale 0 = inner hot core; scale 1 = outer cool edge.
#[inline]
pub fn particle_orbital_soft_edge_scale(beam_index: u32) -> f32 {
    if PARTICLE_ORBITAL_LASER_NUM_BEAMS <= 1 {
        return 0.0;
    }
    let i = beam_index.min(PARTICLE_ORBITAL_LASER_NUM_BEAMS - 1) as f32;
    i / (PARTICLE_ORBITAL_LASER_NUM_BEAMS as f32 - 1.0)
}

/// Soft-edge cylinder width residual for beam index under current width_scalar.
///
/// C++: `width = (inner + scale * (outer - inner)) * widthScale`.
#[inline]
pub fn particle_orbital_soft_edge_width(
    beam_index: u32,
    spawn_frame: u32,
    current_frame: u32,
) -> f32 {
    let scale = particle_orbital_soft_edge_scale(beam_index);
    let base = PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH
        + scale
            * (PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH);
    base * particle_width_scalar(spawn_frame, current_frame)
}

/// Soft-edge alpha residual for beam index (lerps InnerColor.A → OuterColor.A).
#[inline]
pub fn particle_orbital_soft_edge_alpha(beam_index: u32) -> f32 {
    let scale = particle_orbital_soft_edge_scale(beam_index);
    let inner_a = PARTICLE_ORBITAL_LASER_INNER_COLOR.3;
    let outer_a = PARTICLE_ORBITAL_LASER_OUTER_COLOR.3;
    inner_a + scale * (outer_a - inner_a)
}

/// Soft-edge RGB residual for beam index (lerps InnerColor → OuterColor).
///
/// C++ multiplies RGB by innerAlpha on the green/blue/red channels when blending;
/// host residual reports the unpremultiplied color lerp for honesty.
#[inline]
pub fn particle_orbital_soft_edge_color(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_orbital_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_ORBITAL_LASER_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_ORBITAL_LASER_OUTER_COLOR;
    (
        ir + scale * (or - ir),
        ig + scale * (og - ig),
        ib + scale * (ob - ib),
        ia + scale * (oa - ia),
    )
}

/// Soft-edge tile-factor residual for a beam cylinder of given length + width.
///
/// C++: `tileFactor = length / width * textureAspect * TilingScalar` when Tile=Yes.
/// Host residual uses [`PARTICLE_ORBITAL_LASER_TEXTURE_ASPECT`] (fail-closed vs live surface).
#[inline]
pub fn particle_orbital_soft_edge_tile_factor(length: f32, width: f32) -> f32 {
    if !PARTICLE_ORBITAL_LASER_TILE || width <= f32::EPSILON {
        return 1.0;
    }
    (length / width)
        * PARTICLE_ORBITAL_LASER_TEXTURE_ASPECT
        * PARTICLE_ORBITAL_LASER_TILING_SCALAR
}

/// Peak soft-edge outer cylinder width residual (index NumBeams-1 at full scalar).
#[inline]
pub fn particle_orbital_soft_edge_outer_width_peak() -> f32 {
    particle_orbital_soft_edge_width(
        PARTICLE_ORBITAL_LASER_NUM_BEAMS.saturating_sub(1),
        0,
        PARTICLE_WIDTH_GROW_FRAMES,
    )
}

/// Outer-node bone name residual (`FX01`..`FX05`).
#[inline]
pub fn particle_outer_node_bone_name(index: u32) -> String {
    let n = (index % PARTICLE_OUTER_EFFECT_NUM_BONES) + 1;
    format!("{}{:02}", PARTICLE_OUTER_EFFECT_BONE_NAME, n)
}

/// Outer-node residual world position for bone index around building origin.
///
/// Fail-closed: not full W3D bone-world matrix extract / dish mesh attach.
/// Host residual places bones evenly on a ring of
/// [`PARTICLE_OUTER_NODE_RING_RADIUS`] at height [`PARTICLE_OUTER_NODE_RING_HEIGHT`].
#[inline]
pub fn particle_outer_node_bone_position(building_origin: Vec3, index: u32) -> Vec3 {
    let n = PARTICLE_OUTER_EFFECT_NUM_BONES.max(1) as f32;
    let i = (index % PARTICLE_OUTER_EFFECT_NUM_BONES) as f32;
    let angle = (i / n) * std::f32::consts::TAU;
    Vec3::new(
        building_origin.x + angle.cos() * PARTICLE_OUTER_NODE_RING_RADIUS,
        building_origin.y + PARTICLE_OUTER_NODE_RING_HEIGHT,
        building_origin.z + angle.sin() * PARTICLE_OUTER_NODE_RING_RADIUS,
    )
}

/// Connector residual origin (dish connector bone) for STATUS_FIRING residual.
///
/// Fail-closed: not full FXConnector bone matrix; host places connector above origin.
#[inline]
pub fn particle_connector_bone_position(building_origin: Vec3) -> Vec3 {
    Vec3::new(
        building_origin.x,
        building_origin.y + PARTICLE_OUTER_NODE_RING_HEIGHT,
        building_origin.z,
    )
}

/// Retail damage-radius formula honesty residual
/// (`getCurrentLaserRadius() * DamageRadiusScalar`).
///
/// Peak hold = 13 × 3.4 = **44.2**. Host combat still uses
/// [`particle_beam_damage_radius`] (caps at r50 × scalar).
#[inline]
pub fn particle_retail_damage_radius(spawn_frame: u32, current_frame: u32) -> f32 {
    particle_orbital_laser_current_radius(spawn_frame, current_frame)
        * PARTICLE_DAMAGE_RADIUS_SCALAR
}

/// Residual scorch mark radius under ScorchMarkScalar residual.
///
/// Retail: `scorchRadius = getCurrentLaserRadius() * ScorchMarkScalar`.
/// Host residual: full scorch = PARTICLE_BEAM_RADIUS / DamageRadiusScalar
/// * ScorchMarkScalar, scaled by current width scalar.
pub fn particle_scorch_radius(spawn_frame: u32, current_frame: u32) -> f32 {
    let laser_r = if PARTICLE_DAMAGE_RADIUS_SCALAR > 0.0 {
        PARTICLE_BEAM_RADIUS / PARTICLE_DAMAGE_RADIUS_SCALAR
    } else {
        PARTICLE_BEAM_RADIUS
    };
    laser_r * PARTICLE_SCORCH_MARK_SCALAR * particle_width_scalar(spawn_frame, current_frame)
}

/// Next absolute frame for the next scorch mark (fractional residual).
///
/// C++ after each scorch: `nextFactor = scorchMarksMade / totalScorchMarks`,
/// `m_nextScorchMarkFrame = orbitalBirth + nextFactor * orbitalLifetime`.
pub fn particle_next_scorch_frame(spawn_frame: u32, scorch_marks_made: u32) -> u32 {
    if PARTICLE_TOTAL_SCORCH_MARKS == 0 {
        return spawn_frame.saturating_add(PARTICLE_BEAM_DURATION_FRAMES);
    }
    let factor = (scorch_marks_made as f32) / (PARTICLE_TOTAL_SCORCH_MARKS as f32);
    let offset = (factor * (PARTICLE_BEAM_DURATION_FRAMES as f32)).floor() as u32;
    let next = spawn_frame.saturating_add(offset);
    next.max(spawn_frame.saturating_add(1))
}

/// Map scorch mark index onto the SwathOfDeath pulse factor residual.
///
/// Host residual: scorch mark N uses pulse-equivalent index
/// `N * TotalDamagePulses / TotalScorchMarks` so scorches walk the same S-curve.
pub fn particle_scorch_pulse_index(scorch_marks_made_before: u32) -> u32 {
    if PARTICLE_TOTAL_SCORCH_MARKS == 0 {
        return 0;
    }
    ((scorch_marks_made_before as f32) * (PARTICLE_BEAM_TOTAL_PULSES as f32)
        / (PARTICLE_TOTAL_SCORCH_MARKS as f32))
        .floor() as u32
}

// --- Particle Uplink DamagePulseRemnant trail residual ---
// Retail DamagePulseRemnantObjectName = ParticleUplinkCannonTrailRemnant
// (FireWeaponUpdate ParticleUplinkCannonBeamTrailRemnantWeapon + DeletionUpdate).

/// Retail `ParticleUplinkCannonBeamTrailRemnantWeapon` PrimaryDamage.
pub const PARTICLE_REMNANT_DAMAGE_PER_TICK: f32 = 15.0;
/// Retail PrimaryDamageRadius.
pub const PARTICLE_REMNANT_RADIUS: f32 = 10.0;
/// Retail DelayBetweenShots 250 ms → 7 frames @ 30 FPS ((250*30)/1000).
pub const PARTICLE_REMNANT_TICK_INTERVAL_FRAMES: u32 = (250 * 30) / 1000;
/// Retail DeletionUpdate Min/MaxLifetime 4000 ms → 120 frames.
pub const PARTICLE_REMNANT_DURATION_FRAMES: u32 = (4000 * 30) / 1000;
/// Retail remnant Object template name residual (honesty).
pub const PARTICLE_REMNANT_OBJECT_NAME: &str = "ParticleUplinkCannonTrailRemnant";
/// Retail remnant weapon name residual (honesty).
pub const PARTICLE_REMNANT_WEAPON_NAME: &str = "ParticleUplinkCannonBeamTrailRemnantWeapon";

// --- Carpet Bomb line multi-strike residual (retail SUPERWEAPON_CarpetBomb) ---

/// Retail `SUPERWEAPON_CarpetBomb` Payload count (`Payload = CarpetBomb 15`).
pub const CARPET_BOMB_COUNT: u32 = 15;
/// Residual spacing between bomb epicenters along the drop line
/// (host residual; full DeliveryDistance flight path deferred).
pub const CARPET_BOMB_SPACING: f32 = 25.0;
/// Retail OCL CarpetBomb DeliverPayload DropVariance X (C++ horizontal X).
pub const CARPET_BOMB_DROP_VARIANCE_X: f32 = 30.0;
/// Retail OCL CarpetBomb DeliverPayload DropVariance Y (C++ horizontal Y → host Z).
pub const CARPET_BOMB_DROP_VARIANCE_Y: f32 = 40.0;
/// Retail OCL CarpetBomb DropVariance Z (vertical; unused when 0).
pub const CARPET_BOMB_DROP_VARIANCE_Z: f32 = 0.0;
/// Retail OCL CarpetBomb `DropDelay` = 300 ms → 9 frames @ 30 FPS
/// (parseDurationUnsignedInt: ms × 30 / 1000).
pub const CARPET_BOMB_DROP_DELAY_FRAMES: u32 = 9;
/// Retail `CarpetBombWeapon` PrimaryDamage.
pub const CARPET_BOMB_DAMAGE: f32 = 300.0;
/// Retail `CarpetBombWeapon` PrimaryDamageRadius.
pub const CARPET_BOMB_RADIUS: f32 = 50.0;
/// Bomber approach residual frames before first bomb DropDelay stagger starts
/// (fail-closed vs full edge-spawn + transit locomotor).
pub const CARPET_BOMB_IMPACT_DELAY_FRAMES: u32 = 90;

// --- Artillery Barrage scatter multi-shell residual (retail SUPERWEAPON_ArtilleryBarrage1) ---

/// Retail `SUPERWEAPON_ArtilleryBarrage1` FormationSize (Level1).
pub const ARTILLERY_BARRAGE_SHELL_COUNT: u32 = 12;
/// Retail `SUPERWEAPON_ArtilleryBarrage2` FormationSize.
pub const ARTILLERY_BARRAGE_SHELL_COUNT_L2: u32 = 24;
/// Retail `SUPERWEAPON_ArtilleryBarrage3` FormationSize.
pub const ARTILLERY_BARRAGE_SHELL_COUNT_L3: u32 = 36;

/// Residual Artillery Barrage science tier (FormationSize 12/24/36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArtilleryBarrageScienceTier {
    #[default]
    Level1,
    Level2,
    Level3,
}

impl ArtilleryBarrageScienceTier {
    /// Retail FormationSize for this science tier.
    pub fn formation_size(self) -> u32 {
        match self {
            ArtilleryBarrageScienceTier::Level1 => ARTILLERY_BARRAGE_SHELL_COUNT,
            ArtilleryBarrageScienceTier::Level2 => ARTILLERY_BARRAGE_SHELL_COUNT_L2,
            ArtilleryBarrageScienceTier::Level3 => ARTILLERY_BARRAGE_SHELL_COUNT_L3,
        }
    }

    /// Map SCIENCE_ArtilleryBarrage1/2/3 (or generic name residual) to tier.
    /// Higher tiers win when multiple sciences are present (caller should pass highest).
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if n.contains("artillerybarrage3") {
            Some(ArtilleryBarrageScienceTier::Level3)
        } else if n.contains("artillerybarrage2") {
            Some(ArtilleryBarrageScienceTier::Level2)
        } else if n.contains("artillerybarrage1") || n.contains("artillerybarrage") {
            Some(ArtilleryBarrageScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked ArtilleryBarrage science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = ArtilleryBarrageScienceTier::Level1;
        for s in sciences {
            if let Some(t) = Self::from_science_name(s) {
                best = match (best, t) {
                    (_, ArtilleryBarrageScienceTier::Level3)
                    | (ArtilleryBarrageScienceTier::Level3, _) => {
                        ArtilleryBarrageScienceTier::Level3
                    }
                    (_, ArtilleryBarrageScienceTier::Level2)
                    | (ArtilleryBarrageScienceTier::Level2, _) => {
                        ArtilleryBarrageScienceTier::Level2
                    }
                    _ => ArtilleryBarrageScienceTier::Level1,
                };
            }
        }
        best
    }
}
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamage.
pub const ARTILLERY_BARRAGE_DAMAGE: f32 = 105.0;
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamageRadius.
pub const ARTILLERY_BARRAGE_RADIUS: f32 = 50.0;
/// Retail DeliverPayload `WeaponErrorRadius` (shell scatter radius around target).
pub const ARTILLERY_BARRAGE_ERROR_RADIUS: f32 = 100.0;
/// Retail DeliverPayload `DelayDeliveryMax` = 3000 ms → 90 frames @ 30 FPS.
/// Used as: (1) base reaction/approach residual before first shell, and
/// (2) max additional per-shell DelayDelivery stagger after that base.
pub const ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES: u32 = 90;
/// Legacy ring radius used by older residual placement (pre WeaponErrorRadius draw).
/// Kept for honesty/tests that still reference the constant name.
pub const ARTILLERY_BARRAGE_RING_RADIUS: f32 = 75.0;

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
/// Retail SpecialPowerCompletionDie template residual.
pub const SCUD_STORM_MISSILE_SPECIAL_POWER: &str = "SuperweaponScudStorm";

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
    if current_height <= SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET
        && distance_traveled > 0.0
    {
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

// --- Spectre science-tier OrbitTime residual ---

/// Residual Spectre Gunship science tier (OrbitTime 10s / 15s / 20s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SpectreGunshipScienceTier {
    /// Airforce LEVEL1 OrbitTime = 10000 ms → 300 frames.
    Level1,
    #[default]
    /// Default / LEVEL2 OrbitTime = 15000 ms → 450 frames.
    Level2,
    /// Airforce LEVEL3 OrbitTime = 20000 ms → 600 frames.
    Level3,
}

impl SpectreGunshipScienceTier {
    /// Retail OrbitTime residual in logic frames for this science tier.
    pub fn orbit_duration_frames(self) -> u32 {
        match self {
            SpectreGunshipScienceTier::Level1 => 300,
            SpectreGunshipScienceTier::Level2 => SPECTRE_ORBIT_DURATION_FRAMES,
            SpectreGunshipScienceTier::Level3 => 600,
        }
    }

    /// Map SCIENCE_SpectreGunship1/2/3 (or AirF / Solo residual) to tier.
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if n.contains("spectregunship3") {
            Some(SpectreGunshipScienceTier::Level3)
        } else if n.contains("spectregunship2") {
            Some(SpectreGunshipScienceTier::Level2)
        } else if n.contains("spectregunship1")
            || n.contains("spectregunshipsolo")
            || n.contains("spectregunship")
        {
            Some(SpectreGunshipScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked Spectre science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = SpectreGunshipScienceTier::Level2; // retail default OrbitTime 15s
        let mut found = false;
        for s in sciences {
            if let Some(t) = Self::from_science_name(s) {
                if !found {
                    best = t;
                    found = true;
                } else {
                    best = match (best, t) {
                        (_, SpectreGunshipScienceTier::Level3)
                        | (SpectreGunshipScienceTier::Level3, _) => {
                            SpectreGunshipScienceTier::Level3
                        }
                        (_, SpectreGunshipScienceTier::Level2)
                        | (SpectreGunshipScienceTier::Level2, _) => {
                            SpectreGunshipScienceTier::Level2
                        }
                        _ => SpectreGunshipScienceTier::Level1,
                    };
                }
            }
        }
        best
    }
}


/// Host-supported superweapon strike kinds for this residual path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostSuperweaponKind {
    /// USA Daisy Cutter / Fuel Air Bomb / MOAB family.
    DaisyCutter,
    /// USA A-10 Thunderbolt missile strike.
    A10Strike,
    /// GLA SCUD Storm.
    ScudStorm,
    /// China/USA Particle Uplink Cannon continuous beam residual host path.
    ParticleCannon,
    /// China Nuclear Missile / NeutronMissile residual host path.
    NuclearMissile,
    /// GLA Anthrax Bomb residual host path (plane drop + toxin field).
    AnthraxBomb,
    /// USA Spectre Gunship residual host path (delayed orbit + damage ticks).
    SpectreGunship,
    /// Carpet Bomb residual host path (delayed line multi-strike damage).
    CarpetBomb,
    /// China Artillery Barrage residual host path (delayed multi-shell scatter).
    ArtilleryBarrage,
    /// USA Superweapon General Cruise Missile residual host path
    /// (delayed loft + MOABDetonationWeapon area damage).
    CruiseMissile,
}

impl HostSuperweaponKind {
    /// Map a command-system power type to a host residual strike, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::DaisyCutter | SpecialPowerType::FuelAirBomb => {
                Some(HostSuperweaponKind::DaisyCutter)
            }
            SpecialPowerType::Airstrike => Some(HostSuperweaponKind::A10Strike),
            SpecialPowerType::ScudStorm => Some(HostSuperweaponKind::ScudStorm),
            SpecialPowerType::ParticleCannon => Some(HostSuperweaponKind::ParticleCannon),
            SpecialPowerType::NuclearMissile => Some(HostSuperweaponKind::NuclearMissile),
            SpecialPowerType::AnthraxBomb => Some(HostSuperweaponKind::AnthraxBomb),
            SpecialPowerType::SpectreGunship => Some(HostSuperweaponKind::SpectreGunship),
            SpecialPowerType::CarpetBomb => Some(HostSuperweaponKind::CarpetBomb),
            SpecialPowerType::Artillery => Some(HostSuperweaponKind::ArtilleryBarrage),
            SpecialPowerType::CruiseMissile => Some(HostSuperweaponKind::CruiseMissile),
            _ => None,
        }
    }

    /// Human-readable label for logs / honesty reports.
    pub fn label(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutter",
            HostSuperweaponKind::A10Strike => "A10Strike",
            HostSuperweaponKind::ScudStorm => "ScudStorm",
            HostSuperweaponKind::ParticleCannon => "ParticleCannon",
            HostSuperweaponKind::NuclearMissile => "NuclearMissile",
            HostSuperweaponKind::AnthraxBomb => "AnthraxBomb",
            HostSuperweaponKind::SpectreGunship => "SpectreGunship",
            HostSuperweaponKind::CarpetBomb => "CarpetBomb",
            HostSuperweaponKind::ArtilleryBarrage => "ArtilleryBarrage",
            HostSuperweaponKind::CruiseMissile => "CruiseMissile",
        }
    }

    /// Impact delay in logic frames before area damage applies.
    pub fn impact_delay_frames(self) -> u32 {
        match self {
            // FuelAirBombPower residual: impact_delay 3.0s @ 30 FPS.
            HostSuperweaponKind::DaisyCutter => 90,
            // A-10 flight/approach residual (shorter than full aircraft OCL).
            HostSuperweaponKind::A10Strike => 60,
            // SCUD PreAttackDelay residual (first missile); multi-missile stagger follows.
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRE_ATTACK_FRAMES,
            // Particle Uplink charge + beam-travel residual
            // (BeginCharge+RaiseAntenna+ReadyDelay+BeamTravel subset; beam dwell
            // is continuous residual after impact_frame — see HostParticleBeamField).
            HostSuperweaponKind::ParticleCannon => 120,
            // NeutronMissile residual flight/approach (fail-closed vs full
            // NeutronMissileUpdate loft + SpecialSpeedTime path).
            HostSuperweaponKind::NuclearMissile => 180,
            // GLA Jet cargo plane drop residual (same family as DaisyCutter).
            HostSuperweaponKind::AnthraxBomb => 90,
            // Spectre insertion residual (fail-closed vs full edge spawn +
            // transit locomotor + GUNSHIP_STATUS_INSERTING approach).
            HostSuperweaponKind::SpectreGunship => 90,
            // Carpet bomber approach residual (fail-closed vs full edge spawn +
            // DeliverPayload transit + staggered DropDelay).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_IMPACT_DELAY_FRAMES,
            // Retail DelayDeliveryMax = 3000 ms residual (artillerist reaction).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
            // Cruise loft residual (NeutronMissileUpdate family; doors deferred).
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_IMPACT_DELAY_FRAMES,
        }
    }

    /// Max damage at epicenter (host residual values; retail weapon tables deferred).
    pub fn max_damage(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 2000.0,
            HostSuperweaponKind::A10Strike => 500.0,
            // Retail ScudStormDamageWeapon PrimaryDamage (per missile).
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRIMARY_DAMAGE,
            // Continuous beam residual: no one-shot impact blast
            // (damage via HostParticleBeamField pulses — DamagePerSecond 400).
            HostSuperweaponKind::ParticleCannon => 0.0,
            // Retail NeutronMissileSlowDeath Blast6MaxDamage.
            HostSuperweaponKind::NuclearMissile => 3500.0,
            // Retail AnthraxBombWeapon PrimaryDamage (impact blast only).
            HostSuperweaponKind::AnthraxBomb => 200.0,
            // Spectre has no one-shot impact blast; damage is orbit residual only.
            HostSuperweaponKind::SpectreGunship => 0.0,
            // Retail CarpetBombWeapon PrimaryDamage (per bomb epicenter).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_DAMAGE,
            // Retail ArtilleryBarrageDamageWeapon PrimaryDamage (per shell).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_DAMAGE,
            // Retail MOABDetonationWeapon PrimaryDamage (CruiseMissile death weapon).
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_DAMAGE,
        }
    }

    /// Outer damage radius (matches SpecialPower.ini RadiusCursorRadius where known).
    pub fn damage_radius(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 170.0,
            HostSuperweaponKind::A10Strike => 100.0,
            // Retail ScudStormDamageWeapon SecondaryDamageRadius.
            HostSuperweaponKind::ScudStorm => SCUD_STORM_SECONDARY_RADIUS,
            // Residual beam damage radius (see PARTICLE_BEAM_RADIUS).
            HostSuperweaponKind::ParticleCannon => PARTICLE_BEAM_RADIUS,
            // Retail Blast6OuterRadius / DeliveryDecalRadius.
            HostSuperweaponKind::NuclearMissile => 210.0,
            // Retail AnthraxBombWeapon PrimaryDamageRadius.
            HostSuperweaponKind::AnthraxBomb => 100.0,
            // Retail AttackAreaRadius / RadiusCursorRadius.
            HostSuperweaponKind::SpectreGunship => SPECTRE_ORBIT_RADIUS,
            // Retail CarpetBombWeapon PrimaryDamageRadius (per bomb).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_RADIUS,
            // Retail ArtilleryBarrageDamageWeapon PrimaryDamageRadius (per shell).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_RADIUS,
            // Retail MOABDetonationWeapon PrimaryDamageRadius.
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_RADIUS,
        }
    }

    /// Inner radius with full damage (two-stage falloff).
    pub fn falloff_inner(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 100.0,
            HostSuperweaponKind::A10Strike => 40.0,
            // Retail ScudStormDamageWeapon PrimaryDamageRadius (full primary).
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRIMARY_RADIUS,
            // Continuous beam: no one-shot falloff (pulse damage is flat in radius).
            HostSuperweaponKind::ParticleCannon => 0.0,
            // Retail Blast6InnerRadius.
            HostSuperweaponKind::NuclearMissile => 60.0,
            // Flat primary blast (no secondary falloff in weapon table).
            HostSuperweaponKind::AnthraxBomb => 100.0,
            // No impact blast falloff (orbit residual handles damage).
            HostSuperweaponKind::SpectreGunship => 0.0,
            // Flat primary blast per bomb epicenter.
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_RADIUS,
            // Flat primary blast per shell epicenter.
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_RADIUS,
            // Residual two-stage falloff for MOAB primary blast.
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_FALLOFF_INNER,
        }
    }

    /// Whether impact should spawn a residual radiation field.
    pub fn spawns_radiation(self) -> bool {
        matches!(self, HostSuperweaponKind::NuclearMissile)
    }

    /// Whether impact should spawn a residual toxin / anthrax / scud poison field.
    pub fn spawns_toxin_field(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::AnthraxBomb | HostSuperweaponKind::ScudStorm
        )
    }

    /// Whether toxin residual uses ScudStorm LargePoisonField stats (vs Anthrax bomb).
    pub fn spawns_scud_poison_field(self) -> bool {
        matches!(self, HostSuperweaponKind::ScudStorm)
    }

    /// Whether impact should spawn a residual Spectre orbit damage field.
    pub fn spawns_orbit_field(self) -> bool {
        matches!(self, HostSuperweaponKind::SpectreGunship)
    }

    /// Whether impact should spawn a residual Particle Uplink continuous beam field.
    pub fn spawns_beam_field(self) -> bool {
        matches!(self, HostSuperweaponKind::ParticleCannon)
    }

    /// Whether this kind applies multi-point line damage (CarpetBomb residual).
    pub fn is_line_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::CarpetBomb)
    }

    /// Whether this kind applies multi-shell scatter damage (ArtilleryBarrage residual).
    pub fn is_scatter_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::ArtilleryBarrage)
    }

    /// Whether this kind applies multi-missile ScatterTarget residual (ScudStorm).
    pub fn is_scud_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::ScudStorm)
    }

    /// Whether this kind uses multi-point epicenter damage at impact.
    pub fn is_multi_strike(self) -> bool {
        self.is_line_multi_strike()
            || self.is_scatter_multi_strike()
            || self.is_scud_multi_strike()
    }

    /// Whether retail `RadiusDamageAffects` includes ALLIES for the primary blast.
    ///
    /// Host residual previously excluded friendlies fail-closed. Wave 11 closes
    /// ally-hit residual for kinds whose Weapon.ini lists ALLIES.
    pub fn hits_allies(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::DaisyCutter
                | HostSuperweaponKind::A10Strike
                | HostSuperweaponKind::ScudStorm
                | HostSuperweaponKind::NuclearMissile
                | HostSuperweaponKind::AnthraxBomb
                | HostSuperweaponKind::CarpetBomb
                | HostSuperweaponKind::ArtilleryBarrage
                | HostSuperweaponKind::CruiseMissile
            // Continuous Spectre/PUC field paths already have their own team filters.
        )
    }

    /// Whether impact also applies retail `MOABFlameWeapon` secondary residual
    /// (MOABGas SlowDeath MIDPOINT flame — tree-ignite / FLAME damage).
    pub fn spawns_moab_flame(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::DaisyCutter | HostSuperweaponKind::CruiseMissile
        )
    }

    /// Residual multi-point shell/bomb epicenters for multi-strike kinds.
    pub fn multi_strike_points(self, target: Vec3) -> Option<Vec<Vec3>> {
        self.multi_strike_points_with_tier(target, ArtilleryBarrageScienceTier::Level1)
    }

    /// Residual multi-point epicenters with ArtilleryBarrage science-tier FormationSize.
    pub fn multi_strike_points_with_tier(
        self,
        target: Vec3,
        artillery_tier: ArtilleryBarrageScienceTier,
    ) -> Option<Vec<Vec3>> {
        if self.is_line_multi_strike() {
            Some(carpet_bomb_points(target))
        } else if self.is_scatter_multi_strike() {
            Some(artillery_barrage_points_for_tier(target, artillery_tier))
        } else if self.is_scud_multi_strike() {
            Some(scud_storm_points(target))
        } else {
            None
        }
    }

    /// Audio event name queued on activation (host residual).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "SuperweaponDaisyCutter",
            HostSuperweaponKind::A10Strike => "SuperweaponA10Strike",
            HostSuperweaponKind::ScudStorm => "SuperweaponScudStorm",
            HostSuperweaponKind::ParticleCannon => "SuperweaponParticleCannon",
            HostSuperweaponKind::NuclearMissile => "SuperweaponNuclearMissile",
            HostSuperweaponKind::AnthraxBomb => "SuperweaponAnthraxBomb",
            HostSuperweaponKind::SpectreGunship => "SuperweaponSpectreGunship",
            HostSuperweaponKind::CarpetBomb => "SuperweaponCarpetBomb",
            HostSuperweaponKind::ArtilleryBarrage => "SuperweaponArtilleryBarrage",
            // Retail InitiateSound AirRaidSiren residual label for honesty.
            HostSuperweaponKind::CruiseMissile => "SuperweaponCruiseMissile",
        }
    }

    /// Audio event name queued on impact (host residual).
    pub fn impact_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutterExplosion",
            HostSuperweaponKind::A10Strike => "A10StrikeImpact",
            HostSuperweaponKind::ScudStorm => "ScudStormImpact",
            // Beam contact residual (continuous pulses follow).
            HostSuperweaponKind::ParticleCannon => "ParticleCannonBeamStart",
            HostSuperweaponKind::NuclearMissile => "NuclearMissileImpact",
            HostSuperweaponKind::AnthraxBomb => "AnthraxBombImpact",
            // Orbit insertion complete residual (retail SpectreGunshipVoiceArrive).
            HostSuperweaponKind::SpectreGunship => "SpectreGunshipVoiceArrive",
            // Retail ExplosionCarpetBomb residual cue.
            HostSuperweaponKind::CarpetBomb => "ExplosionCarpetBomb",
            // Retail FX_ArtilleryBarrage residual cue.
            HostSuperweaponKind::ArtilleryBarrage => "FX_ArtilleryBarrage",
            // Retail WeaponFX_MOAB_Blast residual cue.
            HostSuperweaponKind::CruiseMissile => "CruiseMissileImpact",
        }
    }
}

/// Residual `WeaponErrorRadius` scatter for artillery formation index.
///
/// C++ `DeliverPayloadNugget` (ObjectCreationList.cpp):
/// ```text
/// if (m_errorRadius > 1.0f && formationIndex > 0) {
///   randomRadius = GameLogicRandomValueReal(0, m_errorRadius);
///   randomAngle  = GameLogicRandomValueReal(0, PI*2);
///   targetPos.x += randomRadius * Cos(randomAngle);
///   targetPos.y += randomRadius * Sin(randomAngle);
/// }
/// ```
/// First formation slot is always spot-on (click target). Host residual uses
/// pure ADC RandomValue algorithm seeded by formation index (re-query stable
/// for multi-strike plan_due recomputes; algorithm parity with
/// GameLogicRandomValueReal — not golden-ratio residual).
/// C++ X/Y horizontal map to host X/Z.
pub fn weapon_error_radius_offset(formation_index: u32, error_radius: f32) -> Vec3 {
    if formation_index == 0 || error_radius <= 1.0 {
        return Vec3::ZERO;
    }
    use super::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(formation_index.wrapping_add(1));
    let radius = s.next_real(0.0, error_radius);
    let angle = s.next_real(0.0, std::f32::consts::TAU);
    Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin())
}

/// Residual `DelayDeliveryMax` frames for artillery formation index.
///
/// C++: `setDisabledUntil(frame + GameLogicRandomValue(0, m_delayDeliveryFramesMax))`
/// (inclusive integer range). Host residual: formationIndex 0 always 0 (lead
/// shell starts after base approach residual); remaining shells draw via pure
/// ADC RandomValue algorithm in `[0, max_frames]` inclusive.
pub fn delay_delivery_frames(formation_index: u32, max_frames: u32) -> u32 {
    if formation_index == 0 || max_frames == 0 {
        return 0;
    }
    use super::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(formation_index.wrapping_add(0xD1));
    // Inclusive [0, max_frames] like GameLogicRandomValue(0, max).
    s.next_int(0, max_frames as i32).max(0) as u32
}

/// Absolute impact frame for artillery shell `formation_index`.
///
/// Base approach residual (`DelayDeliveryMax` as unified reaction) + per-shell
/// DelayDelivery stagger residual.
pub fn artillery_shell_impact_frame(activate_frame: u32, formation_index: u32) -> u32 {
    activate_frame
        .saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES)
        .saturating_add(delay_delivery_frames(
            formation_index,
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
        ))
}

/// Absolute impact frame for carpet bomb index `i` (DropDelay stagger residual).
///
/// First bomb at approach residual; subsequent bombs every `CARPET_BOMB_DROP_DELAY_FRAMES`.
pub fn carpet_bomb_impact_frame(activate_frame: u32, bomb_index: u32) -> u32 {
    activate_frame
        .saturating_add(CARPET_BOMB_IMPACT_DELAY_FRAMES)
        .saturating_add(bomb_index.saturating_mul(CARPET_BOMB_DROP_DELAY_FRAMES))
}

/// Last absolute multi-strike impact frame for a kind (complete residual).
pub fn multi_strike_last_impact_frame(
    kind: HostSuperweaponKind,
    activate_frame: u32,
    artillery_tier: ArtilleryBarrageScienceTier,
) -> u32 {
    if kind.is_scatter_multi_strike() {
        let count = artillery_tier.formation_size().max(1);
        (0..count)
            .map(|i| artillery_shell_impact_frame(activate_frame, i))
            .max()
            .unwrap_or_else(|| activate_frame.saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES))
    } else if kind.is_line_multi_strike() {
        carpet_bomb_impact_frame(activate_frame, CARPET_BOMB_COUNT.saturating_sub(1))
    } else if kind.is_scud_multi_strike() {
        scud_missile_impact_frame(activate_frame, SCUD_STORM_MISSILE_COUNT.saturating_sub(1))
    } else {
        activate_frame.saturating_add(kind.impact_delay_frames())
    }
}

/// Residual DropVariance scatter for bomb index `i`.
///
/// C++ DeliverPayloadAIUpdate:
/// `pos.x += GameLogicRandomValueReal(-var.x, var.x);` (same for y/z when > 0).
/// Host residual: pure ADC RandomValue algorithm seeded by bomb index
/// (re-query stable; algorithm parity with GameLogicRandomValueReal).
/// C++ X/Y horizontal map to host X/Z; C++ Z maps to host Y (vertical).
pub fn drop_variance_offset(index: u32, var_x: f32, var_y: f32, var_z: f32) -> Vec3 {
    use super::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(index.wrapping_add(1));
    let fx = if var_x > 0.0 {
        s.next_real(-var_x, var_x)
    } else {
        0.0
    };
    let fy = if var_y > 0.0 {
        s.next_real(-var_y, var_y)
    } else {
        0.0
    };
    let fz = if var_z > 0.0 {
        s.next_real(-var_z, var_z)
    } else {
        0.0
    };
    // Host Y-up: C++ X → X, C++ Y → Z, C++ Z → Y.
    Vec3::new(fx, fz, fy)
}

/// Build residual bomb epicenters along a line centered on `target`.
///
/// Orientation: east-west (+X) through the target (retail flight path /
/// DeliveryDistance deferred). Line length is
/// `(CARPET_BOMB_COUNT - 1) * CARPET_BOMB_SPACING` centered on target.
/// Each point applies retail DropVariance residual (X:30 Y:40 Z:0) via
/// deterministic host scatter (fail-closed vs GameLogicRandomValueReal).
pub fn carpet_bomb_points(target: Vec3) -> Vec<Vec3> {
    let count = CARPET_BOMB_COUNT.max(1);
    let half = (count as f32 - 1.0) * 0.5;
    let mut points = Vec::with_capacity(count as usize);
    for i in 0..count {
        let offset = (i as f32 - half) * CARPET_BOMB_SPACING;
        let scatter = drop_variance_offset(
            i,
            CARPET_BOMB_DROP_VARIANCE_X,
            CARPET_BOMB_DROP_VARIANCE_Y,
            CARPET_BOMB_DROP_VARIANCE_Z,
        );
        points.push(Vec3::new(
            target.x + offset + scatter.x,
            target.y + scatter.y,
            target.z + scatter.z,
        ));
    }
    points
}

/// Build residual artillery shell epicenters scattered around `target`.
///
/// Formation index 0 is spot-on at the click target (C++). Remaining shells use
/// deterministic `WeaponErrorRadius` residual scatter in `[0, error_radius]`.
/// Shell count is Level1 FormationSize **12** by default.
pub fn artillery_barrage_points(target: Vec3) -> Vec<Vec3> {
    artillery_barrage_points_for_tier(target, ArtilleryBarrageScienceTier::Level1)
}

/// Build residual artillery shell epicenters for a science-tier FormationSize.
///
/// Retail: SUPERWEAPON_ArtilleryBarrage1/2/3 → FormationSize **12 / 24 / 36**.
/// Placement matches C++ `WeaponErrorRadius` residual (not fixed ring).
pub fn artillery_barrage_points_for_tier(
    target: Vec3,
    tier: ArtilleryBarrageScienceTier,
) -> Vec<Vec3> {
    let count = tier.formation_size().max(1);
    let mut points = Vec::with_capacity(count as usize);
    for i in 0..count {
        let off = weapon_error_radius_offset(i, ARTILLERY_BARRAGE_ERROR_RADIUS);
        points.push(Vec3::new(
            target.x + off.x,
            target.y + off.y,
            target.z + off.z,
        ));
    }
    points
}


/// Residual DelayBetweenShots frames for ScudStorm missile index.
///
/// Retail: DelayBetweenShots Min:100 Max:1000 (ms). Host residual: missile 0
/// has no inter-shot delay (PreAttack covers first); remaining missiles draw
/// via pure ADC GameLogicRandomValue algorithm in [min, max] inclusive frames.
pub fn scud_delay_between_frames(missile_index: u32) -> u32 {
    if missile_index == 0 {
        return 0;
    }
    use super::host_rng_residual::HostRandomState;
    let min = SCUD_STORM_DELAY_BETWEEN_MIN_FRAMES as i32;
    let max = SCUD_STORM_DELAY_BETWEEN_MAX_FRAMES as i32;
    let mut s = HostRandomState::seeded(missile_index.wrapping_add(0x5C1D));
    s.next_int(min, max).max(0) as u32
}

/// Absolute impact frame for ScudStorm missile `missile_index`.
///
/// Base PreAttackDelay residual + cumulative DelayBetweenShots stagger.
pub fn scud_missile_impact_frame(activate_frame: u32, missile_index: u32) -> u32 {
    let mut frame = activate_frame.saturating_add(SCUD_STORM_PRE_ATTACK_FRAMES);
    for i in 1..=missile_index {
        frame = frame.saturating_add(scud_delay_between_frames(i));
    }
    frame
}

/// Build residual ScudStorm missile epicenters from retail ScatterTarget table.
///
/// C++: targetPos += ScatterTarget[i] * ScatterTargetScalar (X/Y horizontal).
/// Host residual: C++ X → X, C++ Y → Z. ClipSize 9 entries.
pub fn scud_storm_points(target: Vec3) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(SCUD_STORM_MISSILE_COUNT as usize);
    for (i, &(sx, sy)) in SCUD_STORM_SCATTER_TARGETS.iter().enumerate() {
        if i as u32 >= SCUD_STORM_MISSILE_COUNT {
            break;
        }
        points.push(Vec3::new(
            target.x + sx * SCUD_STORM_SCATTER_SCALAR,
            target.y,
            target.z + sy * SCUD_STORM_SCATTER_SCALAR,
        ));
    }
    // Pad if table shorter than clip (retail falls back to 0,0 for extras).
    while (points.len() as u32) < SCUD_STORM_MISSILE_COUNT {
        points.push(target);
    }
    points
}

/// Lifecycle of a queued host superweapon strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostStrikePhase {
    /// Queued after DoSpecialPower; waiting for impact frame.
    Queued,
    /// Impact resolved; area damage applied.
    Completed,
    /// Cancelled (source died / invalid) before impact.
    Cancelled,
}

/// One pending or completed host superweapon strike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSpecialPowerStrike {
    pub id: u32,
    pub kind: HostSuperweaponKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub impact_frame: u32,
    pub phase: HostStrikePhase,
    /// Total damage dealt across all hit objects at impact.
    pub total_damage_applied: f32,
    /// Number of enemy/neutral objects that received damage.
    pub objects_hit: u32,
    /// Number of objects destroyed by this strike.
    pub objects_destroyed: u32,
    /// ArtilleryBarrage science-tier FormationSize residual (12/24/36).
    /// Ignored for non-artillery kinds. Default Level1.
    #[serde(default)]
    pub artillery_tier: ArtilleryBarrageScienceTier,
    /// SpectreGunship science-tier OrbitTime residual (10s / 15s / 20s).
    /// Ignored for non-Spectre kinds. Default Level2 (retail 15s).
    #[serde(default)]
    pub spectre_tier: SpectreGunshipScienceTier,
    /// ScudStorm anthrax-upgrade residual (Base / Beta / Gamma).
    /// Ignored for non-ScudStorm kinds. Default Base.
    #[serde(default)]
    pub scud_anthrax_tier: ScudStormAnthraxTier,
    /// Multi-strike residual: how many shells/bombs have already applied damage.
    /// One-shot kinds leave this at 0 and complete in a single wave.
    #[serde(default)]
    pub multi_strike_applied: u32,
    /// ParticleCannon intensity-schedule status residual (pre-fire countdown).
    /// Ignored for non-ParticleCannon kinds.
    #[serde(default)]
    pub particle_status: ParticleUplinkStatus,
    /// Highest ParticleCannon status observed (honesty residual).
    #[serde(default)]
    pub particle_status_peak: ParticleUplinkStatus,
    /// ParticleCannon intensity schedule transitions (pre-fire residual).
    #[serde(default)]
    pub particle_intensity_transitions: u32,
    /// Honesty: CHARGING Light outer-node residual applications.
    #[serde(default)]
    pub particle_charging_applications: u32,
    /// Honesty: PREPARING Medium outer-node + UNPACKING model-condition residual.
    #[serde(default)]
    pub particle_preparing_applications: u32,
    /// Honesty: ALMOST_READY Medium connector residual applications.
    #[serde(default)]
    pub particle_almost_ready_applications: u32,
    /// Honesty: READY_TO_FIRE laser-base Light residual applications.
    #[serde(default)]
    pub particle_ready_applications: u32,
    /// Honesty: MODELCONDITION_UNPACKING residual sets (PREPARING).
    #[serde(default)]
    pub particle_model_unpacking_sets: u32,
    /// Honesty: MODELCONDITION_DEPLOYED residual sets (ALMOST_READY/READY/FIRING).
    #[serde(default)]
    pub particle_model_deployed_sets: u32,
    /// Honesty: MODELCONDITION_PACKING residual sets (PACKING).
    #[serde(default)]
    pub particle_model_packing_sets: u32,
    /// ScudStorm PreAttack residual active (PER_CLIP first-missile window).
    #[serde(default)]
    pub scud_pre_attack_active: bool,
    /// Honesty: PreAttack residual frames observed.
    #[serde(default)]
    pub scud_pre_attack_frames: u32,
    /// Honesty: Chem FXBone goo residual systems (FXBone01..03).
    #[serde(default)]
    pub scud_chem_fx_bones: u32,
    /// Honesty: FireFX residual applications (WeaponFX_ScudStormMissile).
    #[serde(default)]
    pub scud_fire_fx_applications: u32,
    /// Honesty: detonation FX residual applications (ScudStormMissileDetonation).
    #[serde(default)]
    pub scud_detonation_fx_applications: u32,
    /// Honesty: launch-bone residual (WeaponA shown during clip).
    #[serde(default)]
    pub scud_launch_bone_applications: u32,
    /// Honesty: ScudStormMissile loft residual applications (MissileAIUpdate path).
    #[serde(default)]
    pub scud_missile_loft_applications: u32,
    /// Honesty: IgnitionFX residual applications (FX_ScudStormIgnition).
    #[serde(default)]
    pub scud_ignition_fx_applications: u32,
    /// Honesty: FireSound residual applications (ScudStormLaunch).
    #[serde(default)]
    pub scud_launch_sound_applications: u32,
    /// Honesty: ProjectileExhaust residual applications (ScudMissileExhaust).
    #[serde(default)]
    pub scud_exhaust_applications: u32,
    /// Honesty: HeightDieUpdate residual applications (TargetHeight 15 / InitialDelay).
    #[serde(default)]
    pub scud_height_die_applications: u32,
    /// Honesty: SpecialPowerCompletionDie residual applications.
    #[serde(default)]
    pub scud_special_power_completion_applications: u32,
    /// Once-at-queue multi-strike OCL residual epicenters (Artillery/Carpet/Scud).
    ///
    /// Drawn via pure ADC at queue time so plan_due reuses the same offsets
    /// (retail once-at-create GameLogic stream residual). Empty for one-shot kinds.
    #[serde(default)]
    pub ocl_points: Vec<Vec3>,
    /// Once-at-queue absolute impact frames per multi-strike shell/bomb/missile.
    #[serde(default)]
    pub ocl_shell_frames: Vec<u32>,
    /// Honesty: once-at-queue OCL residual armed (1 when multi-strike plan stored).
    #[serde(default)]
    pub ocl_once_at_queue_armed: u32,
    /// Honesty: Scud PreferredHeight spawn residual applications.
    #[serde(default)]
    pub scud_spawn_height_applications: u32,
    /// Honesty: PreferredHeight spring residual applications (per missile wave).
    #[serde(default)]
    pub scud_preferred_height_spring_applications: u32,
    /// Honesty: peak loft phase observed (Loft/Turn/Dive/HeightDie residual).
    #[serde(default)]
    pub scud_loft_phase_peak: ScudMissileLoftPhase,
    /// Honesty: last sampled PreferredHeight spring height residual.
    #[serde(default)]
    pub scud_last_spring_height: f32,
    /// Honesty: Scud ballistic flight residual samples (locomotor path).
    #[serde(default)]
    pub scud_ballistic_flight_applications: u32,
    /// Honesty: OnlyWhenMovingDown residual applications.
    #[serde(default)]
    pub scud_only_moving_down_applications: u32,
    /// Honesty: SnapToGroundOnDeath residual applications.
    #[serde(default)]
    pub scud_snap_to_ground_applications: u32,
    /// Honesty: W3DModelDraw model residual applications (`UBScudStrm_M`).
    #[serde(default)]
    pub scud_model_draw_applications: u32,
    /// Honesty: last ballistic flight distance traveled residual.
    #[serde(default)]
    pub scud_last_flight_distance: f32,
    /// Honesty: peak ballistic flight distance residual.
    #[serde(default)]
    pub scud_peak_flight_distance: f32,
    /// Honesty: last ballistic sample height residual (pre-snap).
    #[serde(default)]
    pub scud_last_flight_height: f32,
}

/// Damage application plan for a single victim (computed before mutable apply).
#[derive(Debug, Clone, Copy)]
pub struct HostStrikeDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
}

/// Result of resolving one strike at impact time (or one multi-strike wave).
#[derive(Debug, Clone)]
pub struct HostStrikeImpactPlan {
    pub strike_id: u32,
    pub kind: HostSuperweaponKind,
    pub target_position: Vec3,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostStrikeDamageHit>,
    /// Shell/bomb epicenters applied in this wave (presentation residual).
    pub epicenters: Vec<Vec3>,
    /// How many multi-strike shells/bombs this wave covers.
    pub wave_shell_count: u32,
    /// True when this wave finishes the strike (spawn fields / complete honesty).
    pub is_final_wave: bool,
}

/// Residual radiation field spawned by NuclearMissile impact
/// (`OCL_NukeRadiationField` / `NukeRadiationFieldWeapon` residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostRadiationField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which radiation damage ticks apply.
    pub next_tick_frame: u32,
    /// Total residual radiation damage applied across all ticks.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Objects destroyed by this residual field.
    pub objects_destroyed: u32,
    /// Parent NuclearMissile strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
}

impl HostRadiationField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single radiation victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostRadiationDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub field_id: u32,
}

/// Result of resolving one radiation field's damage tick.
#[derive(Debug, Clone)]
pub struct HostRadiationTickPlan {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub hits: Vec<HostRadiationDamageHit>,
}

/// Residual toxin / anthrax / scud poison field spawned by AnthraxBomb or
/// ScudStorm impact (`OCL_PoisonFieldAnthraxBomb` / `OCL_PoisonFieldLarge` residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostToxinField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which toxin damage ticks apply.
    pub next_tick_frame: u32,
    /// Total residual toxin damage applied across all ticks.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Objects destroyed by this residual field.
    pub objects_destroyed: u32,
    /// Parent strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
    /// Damage per residual tick (Anthrax 40 / Scud LargePoison 15).
    #[serde(default = "default_toxin_damage_per_tick")]
    pub damage_per_tick: f32,
    /// Residual damage radius (Anthrax 300 / Scud LargePoison 140).
    #[serde(default = "default_toxin_radius")]
    pub radius: f32,
    /// Tick interval frames (Anthrax / LargePoison both 15 = 500 ms).
    #[serde(default = "default_toxin_tick_interval")]
    pub tick_interval_frames: u32,
}

fn default_toxin_damage_per_tick() -> f32 {
    ANTHRAX_TOXIN_DAMAGE_PER_TICK
}
fn default_toxin_radius() -> f32 {
    ANTHRAX_TOXIN_RADIUS
}
fn default_toxin_tick_interval() -> u32 {
    ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES
}

impl HostToxinField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single toxin victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostToxinDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub field_id: u32,
}

/// Result of resolving one toxin field's damage tick.
#[derive(Debug, Clone)]
pub struct HostToxinTickPlan {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub hits: Vec<HostToxinDamageHit>,
}

/// Residual Spectre orbit field spawned when gunship reaches target
/// (`SpectreGunshipUpdate` GUNSHIP_STATUS_ORBITING / `SpectreHowitzerGun` residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSpectreOrbitField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which howitzer residual ticks apply.
    pub next_tick_frame: u32,
    /// Next absolute frame at which gattling strafe residual ticks apply.
    #[serde(default)]
    pub next_gattling_tick_frame: u32,
    /// Total residual orbit damage applied across all ticks.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Objects destroyed by this residual field.
    pub objects_destroyed: u32,
    /// Parent SpectreGunship strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
    /// Honesty: howitzer residual ticks applied.
    #[serde(default)]
    pub howitzer_ticks: u32,
    /// Honesty: gattling residual ticks applied.
    #[serde(default)]
    pub gattling_ticks: u32,
    /// Consecutive gattling shots residual (ContinuousFire One/Two ramp).
    #[serde(default)]
    pub gattling_consecutive: u32,
    /// Consecutive howitzer shots residual (ContinuousFire One/Two ramp).
    #[serde(default)]
    pub howitzer_consecutive: u32,
    /// Current gattling continuous-fire level (0 base / 1 mean / 2 fast).
    /// Cleared to base on ContinuousFireCoast cool-down residual.
    #[serde(default)]
    pub gattling_fire_level: u8,
    /// Current howitzer continuous-fire level (0 base / 1 mean / 2 fast).
    /// Cleared to base on ContinuousFireCoast cool-down residual.
    #[serde(default)]
    pub howitzer_fire_level: u8,
    /// Absolute frame after which gattling ContinuousFireCoast cool-down applies.
    #[serde(default)]
    pub gattling_coast_until_frame: u32,
    /// Absolute frame after which howitzer ContinuousFireCoast cool-down applies.
    #[serde(default)]
    pub howitzer_coast_until_frame: u32,
    /// Honesty: gattling ContinuousFireCoast cool-down applications this orbit.
    #[serde(default)]
    pub gattling_coast_applications: u32,
    /// Honesty: howitzer ContinuousFireCoast cool-down applications this orbit.
    #[serde(default)]
    pub howitzer_coast_applications: u32,
    /// Honesty: VoiceRapidFire residual cues when entering FAST (gattling or howitzer).
    #[serde(default)]
    pub rapid_fire_voice_cues: u32,
    /// Honesty: MODELCONDITION_CONTINUOUS_FIRE_MEAN residual sets (FiringTracker::speedUp).
    #[serde(default)]
    pub model_condition_mean_sets: u32,
    /// Honesty: MODELCONDITION_CONTINUOUS_FIRE_FAST residual sets (FiringTracker::speedUp).
    #[serde(default)]
    pub model_condition_fast_sets: u32,
    /// Honesty: MODELCONDITION_CONTINUOUS_FIRE_SLOW residual sets (FiringTracker::coolDown).
    #[serde(default)]
    pub model_condition_slow_sets: u32,
    /// Honesty: SpectreHowitzerShell projectile residual spawns (not full Object).
    #[serde(default)]
    pub howitzer_shells_spawned: u32,
    /// Honesty: SpectreHowitzerGun FireFX residual applications.
    #[serde(default)]
    pub howitzer_shell_fire_fx: u32,
    /// Honesty: SpectreHowitzerShell ProjectileDetonationFX residual applications.
    #[serde(default)]
    pub howitzer_shell_detonation_fx: u32,
    /// Honesty: HeightDie InitialDelay residual applications (pad-safe loft).
    #[serde(default)]
    pub howitzer_shell_height_die_delays: u32,
    /// Honesty: FireSound residual applications (StrategyCenter_ArtilleryRound).
    #[serde(default)]
    pub howitzer_shell_fire_sounds: u32,
    /// Honesty: DumbProjectileBehavior residual applications (per shell).
    #[serde(default)]
    pub howitzer_shell_dumb_projectile_applications: u32,
    /// Honesty: PhysicsBehavior mass residual applications (Mass=1).
    #[serde(default)]
    pub howitzer_shell_physics_mass_applications: u32,
    /// Honesty: InstantDeath DETONATED path residual applications.
    #[serde(default)]
    pub howitzer_shell_death_detonated_applications: u32,
    /// Honesty: InstantDeath LASERED path residual applications (armed).
    #[serde(default)]
    pub howitzer_shell_death_lasered_applications: u32,
    /// Honesty: HeightDie OnlyWhenMovingDown residual applications.
    #[serde(default)]
    pub howitzer_shell_only_moving_down_applications: u32,
    /// Honesty: W3D ModelDraw residual applications (`AVSpectreShell1`).
    #[serde(default)]
    pub howitzer_shell_model_draw_applications: u32,
    /// Honesty: Scale residual applications (0.6).
    #[serde(default)]
    pub howitzer_shell_scale_applications: u32,
    /// Honesty: Shadow residual applications (`SHADOW_DECAL`).
    #[serde(default)]
    pub howitzer_shell_shadow_applications: u32,
    /// Honesty: Geometry residual applications (Cylinder / IsSmall / major+height).
    #[serde(default)]
    pub howitzer_shell_geometry_applications: u32,
    /// Honesty: ActiveBody MaxHealth residual applications.
    #[serde(default)]
    pub howitzer_shell_max_health_applications: u32,
}

impl HostSpectreOrbitField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_howitzer(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }

    pub fn is_due_gattling(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_gattling_tick_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        self.is_due_howitzer(current_frame) || self.is_due_gattling(current_frame)
    }
}

/// Deterministic residual RandomOffsetForHowitzer for howitzer tick index.
///
/// C++: random offset in [-RandomOffsetForHowitzer, +RandomOffsetForHowitzer] on
/// X/Y. Host residual: golden-ratio phase in ±offset (C++ X/Y → host X/Z).
pub fn spectre_howitzer_offset(tick_index: u32) -> Vec3 {
    if SPECTRE_HOWITZER_RANDOM_OFFSET <= 0.0 {
        return Vec3::ZERO;
    }
    let phase = (tick_index as f32 + 1.0) * 0.618_033_988_7;
    let ox = (phase.fract() * 2.0 - 1.0) * SPECTRE_HOWITZER_RANDOM_OFFSET;
    let oz = ((phase + 0.37).fract() * 2.0 - 1.0) * SPECTRE_HOWITZER_RANDOM_OFFSET;
    Vec3::new(ox, 0.0, oz)
}

/// Residual gattling ContinuousFire ROF interval frames for consecutive shots.
///
/// Retail: DelayBetweenShots 100 ms → 3 frames base; CONTINUOUS_FIRE_MEAN 200%
/// → floor(3/2)=1; CONTINUOUS_FIRE_FAST 300% → floor(3/3)=1.
/// Thresholds: ContinuousFireOne=1 / ContinuousFireTwo=2 (exclusive `>`).
pub fn spectre_gattling_interval_frames(consecutive_shots: u32) -> u32 {
    let mult = if consecutive_shots > SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO {
        SPECTRE_GATTLING_FAST_ROF_MULT
    } else if consecutive_shots > SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE {
        SPECTRE_GATTLING_MEAN_ROF_MULT
    } else {
        1.0
    };
    ((SPECTRE_GATTLING_TICK_INTERVAL_FRAMES as f32) / mult)
        .floor()
        .max(1.0) as u32
}

/// Residual howitzer ContinuousFire ROF interval frames for consecutive shots.
///
/// Host base uses HowitzerFiringRate residual **9** frames; MEAN 150% →
/// floor(9/1.5)=6; FAST 200% → floor(9/2)=4.
pub fn spectre_howitzer_interval_frames(consecutive_shots: u32) -> u32 {
    let mult = if consecutive_shots > SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO {
        SPECTRE_HOWITZER_FAST_ROF_MULT
    } else if consecutive_shots > SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE {
        SPECTRE_HOWITZER_MEAN_ROF_MULT
    } else {
        1.0
    };
    ((SPECTRE_ORBIT_TICK_INTERVAL_FRAMES as f32) / mult)
        .floor()
        .max(1.0) as u32
}

/// Damage application plan for a single Spectre orbit victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostSpectreOrbitDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub field_id: u32,
}

/// Result of resolving one Spectre orbit field's damage tick.
#[derive(Debug, Clone)]
pub struct HostSpectreOrbitTickPlan {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub hits: Vec<HostSpectreOrbitDamageHit>,
}

/// Residual Particle Uplink continuous beam field spawned when charge residual
/// completes (`ParticleUplinkCannonUpdate` STATUS_FIRING / TotalDamagePulses).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostParticleBeamField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    /// Click / initial target epicenter residual (swath walks around this).
    pub position: Vec3,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which beam damage pulses apply.
    pub next_tick_frame: u32,
    /// Pulses applied so far (retail TotalDamagePulses cap residual).
    pub pulses_made: u32,
    /// Total residual beam damage applied across all pulses.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×pulse).
    pub damage_applications: u32,
    /// Objects destroyed by this residual beam field.
    pub objects_destroyed: u32,
    /// Parent ParticleCannon strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
    /// Last residual SwathOfDeath epicenter used for a damage pulse.
    #[serde(default)]
    pub last_swath_position: Vec3,
    /// Max |swath offset| seen this beam (honesty for SwathOfDeath residual).
    #[serde(default)]
    pub max_swath_offset: f32,
    /// Honesty: number of pulses that used a non-zero swath offset.
    #[serde(default)]
    pub swath_applications: u32,
    /// Next absolute frame for TotalScorchMarks residual (GroundHitFX + reveal).
    #[serde(default)]
    pub next_scorch_frame: u32,
    /// Scorch marks applied so far (retail TotalScorchMarks cap residual).
    #[serde(default)]
    pub scorch_marks_made: u32,
    /// Honesty: doShroudReveal residual applications (RevealRange).
    #[serde(default)]
    pub reveal_applications: u32,
    /// Honesty: GroundHitFX residual applications (matches scorch cadence).
    #[serde(default)]
    pub ground_hit_fx_applications: u32,
    /// Honesty: peak width scalar reached this beam (WidthGrow residual).
    #[serde(default)]
    pub peak_width_scalar: f32,
    /// Honesty: last residual damage radius used (WidthGrow residual).
    #[serde(default)]
    pub last_damage_radius: f32,
    /// Honesty: last sampled width scalar (grow/hold/decay residual).
    #[serde(default)]
    pub last_width_scalar: f32,
    /// Honesty: lowest width scalar observed during decay phase (starts 1.0).
    #[serde(default = "default_trough_width_scalar")]
    pub trough_width_scalar: f32,
    /// Honesty: frames sampled while in WidthGrow decay (after TotalFiringTime).
    #[serde(default)]
    pub decay_samples: u32,
    /// Last residual scorch epicenter (swath position at scorch).
    #[serde(default)]
    pub last_scorch_position: Vec3,
    /// Honesty: last residual scorch radius.
    #[serde(default)]
    pub last_scorch_radius: f32,
    /// Manual beam driving residual (`setSpecialPowerOverridableDestination`).
    ///
    /// When true, epicenter follows [`current_target_position`] toward
    /// [`override_destination`] instead of SwathOfDeath S-curve (retail human
    /// players always start in manual mode; host residual defaults swath until
    /// an override is applied so AI residual tests stay swath-driven).
    #[serde(default)]
    pub manual_target_mode: bool,
    /// Player-requested beam destination residual.
    #[serde(default)]
    pub override_destination: Vec3,
    /// Live beam target residual (moves toward override at ManualDrivingSpeed).
    #[serde(default)]
    pub current_target_position: Vec3,
    /// Last override click frame (double-click fast-drive residual).
    #[serde(default)]
    pub last_driving_click_frame: u32,
    /// Second-last override click frame.
    #[serde(default)]
    pub second_last_driving_click_frame: u32,
    /// Last frame manual drive advance ran (multi-frame step residual).
    #[serde(default)]
    pub last_drive_update_frame: u32,
    /// Honesty: total horizontal distance driven under manual residual.
    #[serde(default)]
    pub manual_drive_distance_total: f32,
    /// Honesty: number of advance steps that moved the beam.
    #[serde(default)]
    pub manual_drive_applications: u32,
    /// Honesty: advance steps that used ManualFastDrivingSpeed.
    #[serde(default)]
    pub fast_drive_applications: u32,
    /// Honesty: outer-node particle systems created at STATUS_FIRING residual
    /// (retail OuterEffectNumBones × Intense flare).
    #[serde(default)]
    pub outer_node_systems_created: u32,
    /// Honesty: connector lasers created at STATUS_FIRING residual
    /// (retail OuterEffectNumBones × Intense connector laser).
    #[serde(default)]
    pub connector_lasers_created: u32,
    /// Honesty: laser-base flare systems created (STATUS_FIRING Intense).
    #[serde(default)]
    pub laser_base_flare_created: u32,
    /// Honesty: ground-to-orbit orbital laser residual created at STATUS_FIRING.
    #[serde(default)]
    pub ground_to_orbit_laser_created: u32,
    /// Live intensity-schedule status residual (FIRING → POSTFIRE → PACKING).
    #[serde(default)]
    pub status: ParticleUplinkStatus,
    /// Outer-node intensity residual for current status (Light/Medium/Intense).
    #[serde(default)]
    pub outer_intensity: ParticleIntensity,
    /// Connector laser intensity residual for current status.
    #[serde(default)]
    pub connector_intensity: ParticleIntensity,
    /// Laser-base flare intensity residual for current status.
    #[serde(default)]
    pub laser_base_intensity: ParticleIntensity,
    /// Honesty: BeamLaunchFX residual applications (STATUS_FIRING refresh).
    #[serde(default)]
    pub beam_launch_fx_applications: u32,
    /// Next absolute frame for BeamLaunchFX residual refresh.
    #[serde(default)]
    pub next_launch_fx_frame: u32,
    /// Honesty: times status transitioned into POSTFIRE residual.
    #[serde(default)]
    pub postfire_applications: u32,
    /// Honesty: times status transitioned into PACKING residual.
    #[serde(default)]
    pub packing_applications: u32,
    /// Honesty: intensity schedule status transitions observed this beam.
    #[serde(default)]
    pub intensity_transitions: u32,
    /// Honesty: connector flare residual applications (ALMOST_READY+).
    #[serde(default)]
    pub connector_flare_created: u32,
    /// Honesty: peak OuterBeamWidth × width_scalar draw width (visual residual).
    #[serde(default)]
    pub peak_outer_beam_draw_width: f32,
    /// Honesty: last OuterBeamWidth × width_scalar draw width.
    #[serde(default)]
    pub last_outer_beam_draw_width: f32,
    /// Honesty: peak retail getCurrentLaserRadius (OuterBeamWidth×0.5×scalar).
    #[serde(default)]
    pub peak_retail_laser_radius: f32,
    /// Honesty: last retail getCurrentLaserRadius residual.
    #[serde(default)]
    pub last_retail_laser_radius: f32,
    /// Honesty: peak retail damage radius formula (laser radius × DamageRadiusScalar).
    #[serde(default)]
    pub peak_retail_damage_radius: f32,
    /// Honesty: last retail damage radius formula residual.
    #[serde(default)]
    pub last_retail_damage_radius: f32,
    /// Honesty: orbital laser W3DLaserDraw param residual armed at STATUS_FIRING.
    #[serde(default)]
    pub orbital_laser_draw_params_armed: u32,
    /// Honesty: intense connector OuterBeamWidth residual armed at STATUS_FIRING.
    #[serde(default)]
    pub connector_outer_beam_width_armed: u32,
    /// Honesty: multi-beam NumBeams residual armed at STATUS_FIRING (retail 12).
    #[serde(default)]
    pub num_beams_armed: u32,
    /// Honesty: TilingScalar residual armed at STATUS_FIRING.
    #[serde(default)]
    pub tiling_scalar_armed: u32,
    /// Honesty: last ScrollRate UV offset residual (toward muzzle negative).
    #[serde(default)]
    pub last_scroll_uv: f32,
    /// Honesty: peak |ScrollRate UV| residual observed this beam.
    #[serde(default)]
    pub peak_abs_scroll_uv: f32,
    /// Honesty: multi-beam scroll samples taken (sample_width_honesty residual).
    #[serde(default)]
    pub scroll_uv_samples: u32,
    /// Honesty: multi-beam soft-edge residual samples (width/alpha lerp).
    #[serde(default)]
    pub soft_edge_samples: u32,
    /// Honesty: peak soft-edge outer cylinder width residual.
    #[serde(default)]
    pub peak_soft_edge_outer_width: f32,
    /// Honesty: last soft-edge outer cylinder width residual.
    #[serde(default)]
    pub last_soft_edge_outer_width: f32,
    /// Honesty: last soft-edge outer alpha residual.
    #[serde(default)]
    pub last_soft_edge_outer_alpha: f32,
    /// Honesty: last soft-edge tile-factor residual (unit-length outer cylinder).
    #[serde(default)]
    pub last_soft_edge_tile_factor: f32,
    /// Honesty: soft-edge color residual armed (Inner/Outer color constants).
    #[serde(default)]
    pub soft_edge_color_armed: u32,
    /// Honesty: outer-node bone layout residual positions computed.
    #[serde(default)]
    pub outer_node_bone_layout_applications: u32,
    /// Honesty: last outer-node bone residual position (FX01).
    #[serde(default)]
    pub last_outer_node_bone_position: Vec3,
    /// Honesty: connector bone residual position applications.
    #[serde(default)]
    pub connector_bone_layout_applications: u32,
}

fn default_trough_width_scalar() -> f32 {
    1.0
}

impl HostParticleBeamField {
    /// True when the orbital laser has finished the WidthGrow decay tail.
    ///
    /// Beam fields remain alive after TotalDamagePulses / TotalFiringTime so
    /// the decay shrink residual can still be sampled (retail LASERSTATUS_DECAYING).
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    /// True when a damage pulse residual is due.
    ///
    /// Pulses stop once TotalDamagePulses is reached; the field may still live
    /// through the WidthGrow decay tail without further damage ticks.
    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame)
            && self.pulses_made < PARTICLE_BEAM_TOTAL_PULSES
            && current_frame >= self.next_tick_frame
    }

    /// True when a scorch mark residual is due (and marks remain).
    ///
    /// Scorch schedule is independent of damage-pulse cap; it runs for the
    /// beam orbital lifetime (`expires_frame` inclusive), matching retail
    /// STATUS_FIRING scorch cadence through the decay tail.
    pub fn is_due_scorch(&self, current_frame: u32) -> bool {
        self.scorch_marks_made < PARTICLE_TOTAL_SCORCH_MARKS
            && current_frame >= self.next_scorch_frame
            && current_frame < self.expires_frame
    }

    /// Sample WidthGrow grow/hold/decay scalar honesty at `current_frame`.
    pub fn sample_width_honesty(&mut self, current_frame: u32) {
        let width_scalar = particle_width_scalar(self.spawn_frame, current_frame);
        self.last_width_scalar = width_scalar;
        if width_scalar > self.peak_width_scalar {
            self.peak_width_scalar = width_scalar;
        }
        // OuterBeamWidth × scalar draw + retail laser/damage formula residual.
        let draw_w = particle_orbital_laser_draw_width(self.spawn_frame, current_frame);
        self.last_outer_beam_draw_width = draw_w;
        if draw_w > self.peak_outer_beam_draw_width {
            self.peak_outer_beam_draw_width = draw_w;
        }
        let laser_r = particle_orbital_laser_current_radius(self.spawn_frame, current_frame);
        self.last_retail_laser_radius = laser_r;
        if laser_r > self.peak_retail_laser_radius {
            self.peak_retail_laser_radius = laser_r;
        }
        let retail_dmg = particle_retail_damage_radius(self.spawn_frame, current_frame);
        self.last_retail_damage_radius = retail_dmg;
        if retail_dmg > self.peak_retail_damage_radius {
            self.peak_retail_damage_radius = retail_dmg;
        }
        // Multi-beam NumBeams + ScrollRate UV residual (W3DLaserDraw honesty).
        let scroll = particle_orbital_laser_scroll_uv(self.spawn_frame, current_frame);
        self.last_scroll_uv = scroll;
        self.scroll_uv_samples = self.scroll_uv_samples.saturating_add(1);
        let abs_scroll = scroll.abs();
        if abs_scroll > self.peak_abs_scroll_uv {
            self.peak_abs_scroll_uv = abs_scroll;
        }
        // Multi-beam soft-edge width/alpha/tile residual (W3DLaserDraw cylinders).
        let outer_idx = PARTICLE_ORBITAL_LASER_NUM_BEAMS.saturating_sub(1);
        let soft_w =
            particle_orbital_soft_edge_width(outer_idx, self.spawn_frame, current_frame);
        self.last_soft_edge_outer_width = soft_w;
        if soft_w > self.peak_soft_edge_outer_width {
            self.peak_soft_edge_outer_width = soft_w;
        }
        self.last_soft_edge_outer_alpha = particle_orbital_soft_edge_alpha(outer_idx);
        // Unit-length outer cylinder tile-factor residual (aspect × TilingScalar).
        self.last_soft_edge_tile_factor =
            particle_orbital_soft_edge_tile_factor(1.0, soft_w.max(f32::EPSILON));
        self.soft_edge_samples = self.soft_edge_samples.saturating_add(1);
        let decay_start = particle_decay_start_frame(self.spawn_frame);
        if current_frame > decay_start && current_frame < self.expires_frame {
            self.decay_samples = self.decay_samples.saturating_add(1);
            if width_scalar < self.trough_width_scalar {
                self.trough_width_scalar = width_scalar;
            }
        }
    }

    /// Residual damage / scorch epicenter for the current beam mode.
    ///
    /// Manual mode uses live `current_target_position`; swath mode uses the
    /// S-curve offset for the given pulse index.
    pub fn residual_epicenter(&self, pulse_index: u32) -> Vec3 {
        if self.manual_target_mode {
            self.current_target_position
        } else {
            particle_swath_epicenter(self.position, pulse_index)
        }
    }
}

/// Damage application plan for a single Particle Uplink beam victim this pulse.
#[derive(Debug, Clone, Copy)]
pub struct HostParticleBeamDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub field_id: u32,
}

/// Result of resolving one Particle Uplink beam field's damage pulse.
#[derive(Debug, Clone)]
pub struct HostParticleBeamTickPlan {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub hits: Vec<HostParticleBeamDamageHit>,
    /// Residual WidthGrow damage radius used for this pulse.
    pub damage_radius: f32,
    /// Residual width scalar used for this pulse.
    pub width_scalar: f32,
}

/// Result of resolving one Particle Uplink scorch / reveal residual event.
#[derive(Debug, Clone)]
pub struct HostParticleScorchRevealEvent {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub scorch_radius: f32,
    pub reveal_range: f32,
    pub scorch_mark_index: u32,
}

/// Residual DamagePulseRemnant trail field (`ParticleUplinkCannonTrailRemnant`).
///
/// Retail: each beam damage pulse spawns an immortal remnant Object with
/// FireWeaponUpdate (PrimaryDamage 15 / radius 10 / DelayBetweenShots 250 ms)
/// and DeletionUpdate lifetime 4000 ms. Host residual is a compact field that
/// ticks residual PARTICLE_BEAM damage at the pulse epicenter (fail-closed vs
/// full ThingFactory Object / ImmortalBody / DeletionUpdate module stack).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostParticleRemnantField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    /// Pulse epicenter residual (swath position at spawn).
    pub position: Vec3,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which remnant damage ticks apply.
    pub next_tick_frame: u32,
    /// Total residual remnant damage applied across all ticks.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Objects destroyed by this residual remnant field.
    pub objects_destroyed: u32,
    /// Parent ParticleCannon beam field id (0 if spawned without a beam).
    pub parent_beam_id: u32,
    /// Parent ParticleCannon strike id (0 if unknown).
    pub parent_strike_id: u32,
}

impl HostParticleRemnantField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single remnant trail victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostParticleRemnantDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub field_id: u32,
}

/// Result of resolving one remnant trail field's damage tick.
#[derive(Debug, Clone)]
pub struct HostParticleRemnantTickPlan {
    pub field_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub hits: Vec<HostParticleRemnantDamageHit>,
}

/// Host registry of superweapon strikes that queue and complete.
#[derive(Debug, Clone, Default)]
pub struct HostSpecialPowerStrikeRegistry {
    next_id: u32,
    strikes: HashMap<u32, HostSpecialPowerStrike>,
    /// Strikes that completed impact this frame (presentation / honesty drain).
    completed_this_frame: Vec<u32>,
    /// Strikes activated this frame.
    activated_this_frame: Vec<u32>,
    /// Active residual radiation fields (NuclearMissile impact residual).
    radiation_fields: Vec<HostRadiationField>,
    next_radiation_id: u32,
    /// Radiation fields spawned this frame (honesty / presentation drain).
    radiation_spawned_this_frame: Vec<u32>,
    /// Lifetime count of radiation fields spawned (survives prune; honesty).
    radiation_fields_spawned_total: u32,
    /// Lifetime radiation damage applications (honesty after field expiry).
    radiation_damage_applications_total: u32,
    /// Active residual toxin fields (AnthraxBomb impact residual).
    toxin_fields: Vec<HostToxinField>,
    next_toxin_id: u32,
    /// Toxin fields spawned this frame (honesty / presentation drain).
    toxin_spawned_this_frame: Vec<u32>,
    /// Lifetime count of toxin fields spawned (survives prune; honesty).
    toxin_fields_spawned_total: u32,
    /// Lifetime toxin damage applications (honesty after field expiry).
    toxin_damage_applications_total: u32,
    /// Active residual Spectre orbit fields (SpectreGunship residual).
    orbit_fields: Vec<HostSpectreOrbitField>,
    next_orbit_id: u32,
    /// Orbit fields spawned this frame (honesty / presentation drain).
    orbit_spawned_this_frame: Vec<u32>,
    /// Lifetime count of orbit fields spawned (survives prune; honesty).
    orbit_fields_spawned_total: u32,
    /// Lifetime orbit damage applications (honesty after field expiry).
    orbit_damage_applications_total: u32,
    /// Active residual Particle Uplink continuous beam fields.
    beam_fields: Vec<HostParticleBeamField>,
    next_beam_id: u32,
    /// Beam fields spawned this frame (honesty / presentation drain).
    beam_spawned_this_frame: Vec<u32>,
    /// Lifetime count of beam fields spawned (survives prune; honesty).
    beam_fields_spawned_total: u32,
    /// Lifetime beam damage applications (honesty after field expiry).
    beam_damage_applications_total: u32,
    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    remnant_fields: Vec<HostParticleRemnantField>,
    next_remnant_id: u32,
    /// Remnant fields spawned this frame (honesty / presentation drain).
    remnant_spawned_this_frame: Vec<u32>,
    /// Lifetime count of remnant fields spawned (survives prune; honesty).
    remnant_fields_spawned_total: u32,
    /// Lifetime remnant damage applications (honesty after field expiry).
    remnant_damage_applications_total: u32,
}

impl HostSpecialPowerStrikeRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            strikes: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
            radiation_fields: Vec::new(),
            next_radiation_id: 1,
            radiation_spawned_this_frame: Vec::new(),
            radiation_fields_spawned_total: 0,
            radiation_damage_applications_total: 0,
            toxin_fields: Vec::new(),
            next_toxin_id: 1,
            toxin_spawned_this_frame: Vec::new(),
            toxin_fields_spawned_total: 0,
            toxin_damage_applications_total: 0,
            orbit_fields: Vec::new(),
            next_orbit_id: 1,
            orbit_spawned_this_frame: Vec::new(),
            orbit_fields_spawned_total: 0,
            orbit_damage_applications_total: 0,
            beam_fields: Vec::new(),
            next_beam_id: 1,
            beam_spawned_this_frame: Vec::new(),
            beam_fields_spawned_total: 0,
            beam_damage_applications_total: 0,
            remnant_fields: Vec::new(),
            next_remnant_id: 1,
            remnant_spawned_this_frame: Vec::new(),
            remnant_fields_spawned_total: 0,
            remnant_damage_applications_total: 0,
        }
    }

    pub fn clear(&mut self) {
        self.strikes.clear();
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.radiation_fields.clear();
        self.radiation_spawned_this_frame.clear();
        self.next_id = 1;
        self.next_radiation_id = 1;
        self.radiation_fields_spawned_total = 0;
        self.radiation_damage_applications_total = 0;
        self.toxin_fields.clear();
        self.toxin_spawned_this_frame.clear();
        self.next_toxin_id = 1;
        self.toxin_fields_spawned_total = 0;
        self.toxin_damage_applications_total = 0;
        self.orbit_fields.clear();
        self.orbit_spawned_this_frame.clear();
        self.next_orbit_id = 1;
        self.orbit_fields_spawned_total = 0;
        self.orbit_damage_applications_total = 0;
        self.beam_fields.clear();
        self.beam_spawned_this_frame.clear();
        self.next_beam_id = 1;
        self.beam_fields_spawned_total = 0;
        self.beam_damage_applications_total = 0;
        self.remnant_fields.clear();
        self.remnant_spawned_this_frame.clear();
        self.next_remnant_id = 1;
        self.remnant_fields_spawned_total = 0;
        self.remnant_damage_applications_total = 0;
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.radiation_spawned_this_frame.clear();
        self.toxin_spawned_this_frame.clear();
        self.orbit_spawned_this_frame.clear();
        self.beam_spawned_this_frame.clear();
        self.remnant_spawned_this_frame.clear();
    }

    /// Allocator cursor for next strike id (survives save/load).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Allocator cursor for next radiation field id (survives save/load).
    pub fn next_radiation_id(&self) -> u32 {
        self.next_radiation_id
    }

    /// Active residual radiation fields (NuclearMissile).
    pub fn radiation_fields(&self) -> &[HostRadiationField] {
        &self.radiation_fields
    }

    pub fn radiation_spawned_this_frame(&self) -> &[u32] {
        &self.radiation_spawned_this_frame
    }

    /// Allocator cursor for next toxin field id (survives save/load).
    pub fn next_toxin_id(&self) -> u32 {
        self.next_toxin_id
    }

    /// Active residual toxin fields (AnthraxBomb).
    pub fn toxin_fields(&self) -> &[HostToxinField] {
        &self.toxin_fields
    }

    pub fn toxin_spawned_this_frame(&self) -> &[u32] {
        &self.toxin_spawned_this_frame
    }

    /// Allocator cursor for next Spectre orbit field id (survives save/load).
    pub fn next_orbit_id(&self) -> u32 {
        self.next_orbit_id
    }

    /// Active residual Spectre orbit fields (SpectreGunship).
    pub fn orbit_fields(&self) -> &[HostSpectreOrbitField] {
        &self.orbit_fields
    }

    pub fn orbit_spawned_this_frame(&self) -> &[u32] {
        &self.orbit_spawned_this_frame
    }

    /// Allocator cursor for next Particle Uplink beam field id (save/load).
    pub fn next_beam_id(&self) -> u32 {
        self.next_beam_id
    }

    /// Active residual Particle Uplink continuous beam fields.
    pub fn beam_fields(&self) -> &[HostParticleBeamField] {
        &self.beam_fields
    }

    pub fn beam_spawned_this_frame(&self) -> &[u32] {
        &self.beam_spawned_this_frame
    }

    /// Allocator cursor for next Particle Uplink remnant field id (save/load).
    pub fn next_remnant_id(&self) -> u32 {
        self.next_remnant_id
    }

    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    pub fn remnant_fields(&self) -> &[HostParticleRemnantField] {
        &self.remnant_fields
    }

    pub fn remnant_spawned_this_frame(&self) -> &[u32] {
        &self.remnant_spawned_this_frame
    }

    /// Replace registry contents from a save/load snapshot.
    ///
    /// Frame-local presentation drains (`activated_this_frame` /
    /// `completed_this_frame` / `radiation_spawned_this_frame` /
    /// `toxin_spawned_this_frame` / `orbit_spawned_this_frame` /
    /// `beam_spawned_this_frame`) are cleared — they are not persistent.
    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
    ) {
        self.restore_from_snapshot_with_residuals(
            next_id,
            strikes,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
        );
    }

    /// Replace registry including residual radiation fields (save/load).
    pub fn restore_from_snapshot_with_radiation(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
        next_radiation_id: u32,
        radiation_fields: impl IntoIterator<Item = HostRadiationField>,
        radiation_fields_spawned_total: u32,
        radiation_damage_applications_total: u32,
    ) {
        self.restore_from_snapshot_with_residuals(
            next_id,
            strikes,
            next_radiation_id,
            radiation_fields,
            radiation_fields_spawned_total,
            radiation_damage_applications_total,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
            1,
            Vec::new(),
            0,
            0,
        );
    }

    /// Replace registry including radiation + toxin + Spectre orbit + PUC beam
    /// residual fields (save/load).
    #[allow(clippy::too_many_arguments)]
    pub fn restore_from_snapshot_with_residuals(
        &mut self,
        next_id: u32,
        strikes: impl IntoIterator<Item = HostSpecialPowerStrike>,
        next_radiation_id: u32,
        radiation_fields: impl IntoIterator<Item = HostRadiationField>,
        radiation_fields_spawned_total: u32,
        radiation_damage_applications_total: u32,
        next_toxin_id: u32,
        toxin_fields: impl IntoIterator<Item = HostToxinField>,
        toxin_fields_spawned_total: u32,
        toxin_damage_applications_total: u32,
        next_orbit_id: u32,
        orbit_fields: impl IntoIterator<Item = HostSpectreOrbitField>,
        orbit_fields_spawned_total: u32,
        orbit_damage_applications_total: u32,
        next_beam_id: u32,
        beam_fields: impl IntoIterator<Item = HostParticleBeamField>,
        beam_fields_spawned_total: u32,
        beam_damage_applications_total: u32,
        next_remnant_id: u32,
        remnant_fields: impl IntoIterator<Item = HostParticleRemnantField>,
        remnant_fields_spawned_total: u32,
        remnant_damage_applications_total: u32,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for strike in strikes {
            max_id = max_id.max(strike.id);
            self.strikes.insert(strike.id, strike);
        }
        // Prefer the saved allocator; never reuse an id that is already present.
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);

        let mut max_rad = 0_u32;
        for field in radiation_fields {
            max_rad = max_rad.max(field.id);
            self.radiation_fields.push(field);
        }
        self.next_radiation_id = next_radiation_id.max(max_rad.saturating_add(1)).max(1);
        self.radiation_fields_spawned_total = radiation_fields_spawned_total.max(max_rad);
        self.radiation_damage_applications_total = radiation_damage_applications_total;

        let mut max_tox = 0_u32;
        for field in toxin_fields {
            max_tox = max_tox.max(field.id);
            self.toxin_fields.push(field);
        }
        self.next_toxin_id = next_toxin_id.max(max_tox.saturating_add(1)).max(1);
        self.toxin_fields_spawned_total = toxin_fields_spawned_total.max(max_tox);
        self.toxin_damage_applications_total = toxin_damage_applications_total;

        let mut max_orb = 0_u32;
        for field in orbit_fields {
            max_orb = max_orb.max(field.id);
            self.orbit_fields.push(field);
        }
        self.next_orbit_id = next_orbit_id.max(max_orb.saturating_add(1)).max(1);
        self.orbit_fields_spawned_total = orbit_fields_spawned_total.max(max_orb);
        self.orbit_damage_applications_total = orbit_damage_applications_total;

        let mut max_beam = 0_u32;
        for field in beam_fields {
            max_beam = max_beam.max(field.id);
            self.beam_fields.push(field);
        }
        self.next_beam_id = next_beam_id.max(max_beam.saturating_add(1)).max(1);
        self.beam_fields_spawned_total = beam_fields_spawned_total.max(max_beam);
        self.beam_damage_applications_total = beam_damage_applications_total;

        let mut max_rem = 0_u32;
        for field in remnant_fields {
            max_rem = max_rem.max(field.id);
            self.remnant_fields.push(field);
        }
        self.next_remnant_id = next_remnant_id.max(max_rem.saturating_add(1)).max(1);
        self.remnant_fields_spawned_total = remnant_fields_spawned_total.max(max_rem);
        self.remnant_damage_applications_total = remnant_damage_applications_total;
    }

    pub fn radiation_fields_spawned_total(&self) -> u32 {
        self.radiation_fields_spawned_total
    }

    pub fn radiation_damage_applications_total(&self) -> u32 {
        self.radiation_damage_applications_total
    }

    pub fn toxin_fields_spawned_total(&self) -> u32 {
        self.toxin_fields_spawned_total
    }

    pub fn toxin_damage_applications_total(&self) -> u32 {
        self.toxin_damage_applications_total
    }

    pub fn orbit_fields_spawned_total(&self) -> u32 {
        self.orbit_fields_spawned_total
    }

    pub fn orbit_damage_applications_total(&self) -> u32 {
        self.orbit_damage_applications_total
    }

    pub fn beam_fields_spawned_total(&self) -> u32 {
        self.beam_fields_spawned_total
    }

    pub fn beam_damage_applications_total(&self) -> u32 {
        self.beam_damage_applications_total
    }

    pub fn remnant_fields_spawned_total(&self) -> u32 {
        self.remnant_fields_spawned_total
    }

    pub fn remnant_damage_applications_total(&self) -> u32 {
        self.remnant_damage_applications_total
    }

    pub fn strike_count(&self) -> usize {
        self.strikes.len()
    }

    pub fn pending_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostSpecialPowerStrike> {
        self.strikes.get(&id)
    }

    pub fn strikes_snapshot(&self) -> Vec<HostSpecialPowerStrike> {
        let mut v: Vec<_> = self.strikes.values().cloned().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued && s.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed && s.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Queue a superweapon strike. Returns host strike id.
    /// ArtilleryBarrage uses Level1 FormationSize (12) by default.
    pub fn queue(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        self.queue_with_artillery_tier(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            ArtilleryBarrageScienceTier::Level1,
        )
    }

    /// Queue a superweapon strike with ArtilleryBarrage science-tier FormationSize.
    pub fn queue_with_artillery_tier(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
    ) -> u32 {
        self.queue_with_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            artillery_tier,
            SpectreGunshipScienceTier::Level2,
        )
    }

    /// Queue a superweapon strike with Artillery FormationSize + Spectre OrbitTime tiers.
    pub fn queue_with_tiers(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
        spectre_tier: SpectreGunshipScienceTier,
    ) -> u32 {
        self.queue_with_all_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            artillery_tier,
            spectre_tier,
            ScudStormAnthraxTier::Base,
        )
    }

    /// Queue with Artillery + Spectre + ScudStorm anthrax residual tiers.
    pub fn queue_with_all_tiers(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        artillery_tier: ArtilleryBarrageScienceTier,
        spectre_tier: SpectreGunshipScienceTier,
        scud_anthrax_tier: ScudStormAnthraxTier,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        // First multi-strike shell/bomb/missile due frame residual.
        let impact_frame = activate_frame.saturating_add(kind.impact_delay_frames());
        let mut strike = HostSpecialPowerStrike {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            impact_frame,
            phase: HostStrikePhase::Queued,
            total_damage_applied: 0.0,
            objects_hit: 0,
            objects_destroyed: 0,
            artillery_tier,
            spectre_tier,
            scud_anthrax_tier,
            multi_strike_applied: 0,
            particle_status: ParticleUplinkStatus::Idle,
            particle_status_peak: ParticleUplinkStatus::Idle,
            particle_intensity_transitions: 0,
            particle_charging_applications: 0,
            particle_preparing_applications: 0,
            particle_almost_ready_applications: 0,
            particle_ready_applications: 0,
            particle_model_unpacking_sets: 0,
            particle_model_deployed_sets: 0,
            particle_model_packing_sets: 0,
            scud_pre_attack_active: false,
            scud_pre_attack_frames: 0,
            scud_chem_fx_bones: 0,
            scud_fire_fx_applications: 0,
            scud_detonation_fx_applications: 0,
            scud_launch_bone_applications: 0,
            scud_missile_loft_applications: 0,
            scud_ignition_fx_applications: 0,
            scud_launch_sound_applications: 0,
            scud_exhaust_applications: 0,
            scud_height_die_applications: 0,
            scud_special_power_completion_applications: 0,
            ocl_points: Vec::new(),
            ocl_shell_frames: Vec::new(),
            ocl_once_at_queue_armed: 0,
            scud_spawn_height_applications: 0,
            scud_preferred_height_spring_applications: 0,
            scud_loft_phase_peak: ScudMissileLoftPhase::Loft,
            scud_last_spring_height: 0.0,
            scud_ballistic_flight_applications: 0,
            scud_only_moving_down_applications: 0,
            scud_snap_to_ground_applications: 0,
            scud_model_draw_applications: 0,
            scud_last_flight_distance: 0.0,
            scud_peak_flight_distance: 0.0,
            scud_last_flight_height: 0.0,
        };
        // Once-at-queue multi-strike OCL residual: store epicenters + shell
        // frames so plan_due reuses the same ADC draws (retail once-at-create).
        if kind.is_multi_strike() {
            let points = kind
                .multi_strike_points_with_tier(target_position, artillery_tier)
                .unwrap_or_default();
            let mut frames = Vec::with_capacity(points.len());
            for i in 0..points.len() as u32 {
                let shell_frame = if kind.is_scatter_multi_strike() {
                    artillery_shell_impact_frame(activate_frame, i)
                } else if kind.is_scud_multi_strike() {
                    scud_missile_impact_frame(activate_frame, i)
                } else {
                    carpet_bomb_impact_frame(activate_frame, i)
                };
                frames.push(shell_frame);
            }
            strike.ocl_points = points;
            strike.ocl_shell_frames = frames;
            strike.ocl_once_at_queue_armed = 1;
        }
        // Seed ParticleCannon pre-fire intensity residual at activate frame.
        if kind == HostSuperweaponKind::ParticleCannon {
            apply_particle_charge_status(&mut strike, activate_frame);
        }
        // Seed ScudStorm PreAttack + Chem FX residual at activate.
        if kind == HostSuperweaponKind::ScudStorm {
            strike.scud_pre_attack_active = true;
            strike.scud_chem_fx_bones = SCUD_STORM_CHEM_FX_BONE_COUNT;
            strike.scud_launch_bone_applications = 1;
        }
        self.strikes.insert(id, strike);
        self.activated_this_frame.push(id);
        id
    }

    /// Compute falloff damage for distance from epicenter.
    ///
    /// ScudStorm residual uses retail primary/secondary step damage
    /// (`ScudStormDamageWeapon`): full Primary inside PrimaryRadius, Secondary
    /// out to SecondaryRadius (not linear falloff).
    pub fn damage_at_distance(kind: HostSuperweaponKind, distance: f32) -> f32 {
        Self::damage_at_distance_with_scud_tier(kind, distance, ScudStormAnthraxTier::Base)
    }

    /// Falloff residual with ScudStorm anthrax-upgrade tier (Secondary 150/200, Primary 500/550).
    pub fn damage_at_distance_with_scud_tier(
        kind: HostSuperweaponKind,
        distance: f32,
        scud_tier: ScudStormAnthraxTier,
    ) -> f32 {
        if kind == HostSuperweaponKind::ScudStorm {
            if distance <= SCUD_STORM_PRIMARY_RADIUS {
                return scud_tier.primary_damage();
            }
            if distance <= SCUD_STORM_SECONDARY_RADIUS {
                return scud_tier.secondary_damage();
            }
            return 0.0;
        }
        let radius = kind.damage_radius();
        let inner = kind.falloff_inner();
        let max = kind.max_damage();
        if distance <= inner {
            max
        } else if distance >= radius {
            0.0
        } else {
            let range = (radius - inner).max(f32::EPSILON);
            let t = (distance - inner) / range;
            max * (1.0 - t).max(0.0)
        }
    }

    /// Build impact damage plans for all strikes whose impact frame has arrived.
    /// Does not mutate object health — GameLogic applies hits.
    ///
    /// Multi-strike residuals (CarpetBomb line / ArtilleryBarrage scatter):
    /// - Shells/bombs apply on their DelayDelivery / DropDelay residual frames.
    /// - Each living enemy takes the max damage from any **due this wave**
    ///   epicenter (not a single circular blast at the click point only).
    /// - Jumping past several stagger frames applies all overdue shells/bombs
    ///   in one wave (save/load and host tests).
    pub fn plan_due_impacts(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostStrikeImpactPlan> {
        let mut plans = Vec::new();
        for strike in self.strikes.values() {
            if strike.phase != HostStrikePhase::Queued || current_frame < strike.impact_frame {
                continue;
            }

            let (due_points, wave_shell_count, is_final_wave) = if strike.kind.is_multi_strike() {
                // Prefer once-at-queue residual plan (stored ADC draws); fall back
                // to re-query for older snapshots without ocl_points.
                let all_points = if !strike.ocl_points.is_empty() {
                    strike.ocl_points.clone()
                } else {
                    strike
                        .kind
                        .multi_strike_points_with_tier(
                            strike.target_position,
                            strike.artillery_tier,
                        )
                        .unwrap_or_default()
                };
                let total = all_points.len() as u32;
                if total == 0 || strike.multi_strike_applied >= total {
                    continue;
                }
                let mut due = Vec::new();
                let mut due_count = 0_u32;
                for (i, p) in all_points.iter().enumerate() {
                    let idx = i as u32;
                    if idx < strike.multi_strike_applied {
                        continue;
                    }
                    let shell_frame = if let Some(&f) = strike.ocl_shell_frames.get(i) {
                        f
                    } else if strike.kind.is_scatter_multi_strike() {
                        artillery_shell_impact_frame(strike.activate_frame, idx)
                    } else if strike.kind.is_scud_multi_strike() {
                        scud_missile_impact_frame(strike.activate_frame, idx)
                    } else {
                        carpet_bomb_impact_frame(strike.activate_frame, idx)
                    };
                    if shell_frame <= current_frame {
                        due.push(*p);
                        due_count = due_count.saturating_add(1);
                    }
                }
                if due_count == 0 {
                    continue;
                }
                let applied_after = strike.multi_strike_applied.saturating_add(due_count);
                let is_final = applied_after >= total;
                (due, due_count, is_final)
            } else {
                (
                    vec![strike.target_position],
                    1,
                    true,
                )
            };

            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == strike.source_object {
                    continue;
                }
                // Retail RadiusDamageAffects ALLIES residual (wave 11).
                // Kinds without ALLIES still exclude same-team friendlies.
                if team == strike.source_team && !strike.kind.hits_allies() {
                    continue;
                }
                let dmg = if strike.kind.is_multi_strike() {
                    // Multi-strike wave: best (nearest) due shell/bomb epicenter.
                    due_points
                        .iter()
                        .map(|epicenter| {
                            Self::damage_at_distance_with_scud_tier(
                                strike.kind,
                                horizontal_distance(pos, *epicenter),
                                strike.scud_anthrax_tier,
                            )
                        })
                        .fold(0.0_f32, f32::max)
                } else {
                    let dist = horizontal_distance(pos, strike.target_position);
                    let primary = Self::damage_at_distance_with_scud_tier(strike.kind, dist, strike.scud_anthrax_tier);
                    // MOABFlameWeapon secondary residual (DaisyCutter / CruiseMissile).
                    // Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state.
                    let flame = if strike.kind.spawns_moab_flame() && dist <= MOAB_FLAME_RADIUS {
                        MOAB_FLAME_DAMAGE
                    } else {
                        0.0
                    };
                    primary + flame
                };
                if dmg > 0.0 {
                    hits.push(HostStrikeDamageHit {
                        target_id: id,
                        damage: dmg,
                    });
                }
            }
            // Presentation epicenter: first due point (or strike target).
            let present_pos = due_points
                .first()
                .copied()
                .unwrap_or(strike.target_position);
            plans.push(HostStrikeImpactPlan {
                strike_id: strike.id,
                kind: strike.kind,
                target_position: present_pos,
                source_object: strike.source_object,
                source_team: strike.source_team,
                hits,
                epicenters: due_points,
                wave_shell_count,
                is_final_wave,
            });
        }
        plans.sort_by_key(|p| p.strike_id);
        plans
    }

    /// Record impact results after GameLogic applied damage.
    ///
    /// Multi-strike waves accumulate damage and only complete on `is_final_wave`.
    /// For `NuclearMissile`, also spawns a residual radiation field at the
    /// epicenter (retail `OCL_NukeRadiationField` residual).
    /// For `AnthraxBomb`, also spawns a residual toxin field at the epicenter
    /// (retail `OCL_PoisonFieldAnthraxBomb` residual).
    /// For `SpectreGunship`, also spawns a residual orbit field at the target
    /// (retail `SpectreGunshipUpdate` ORBITING residual).
    /// For `ParticleCannon`, also spawns a residual continuous beam field at
    /// the target (retail `ParticleUplinkCannonUpdate` STATUS_FIRING residual).
    pub fn record_impact_complete(
        &mut self,
        strike_id: u32,
        total_damage: f32,
        objects_hit: u32,
        objects_destroyed: u32,
    ) {
        // Default: treat as final single wave (legacy callers).
        self.record_impact_wave(
            strike_id,
            total_damage,
            objects_hit,
            objects_destroyed,
            1,
            true,
            &[],
        );
    }

    /// Record one multi-strike impact wave (or a one-shot final impact).
    ///
    /// `epicenters` carries this wave's shell/missile impact points so ScudStorm
    /// can spawn per-missile LargePoisonField residual (retail FireOCL each detonation).
    pub fn record_impact_wave(
        &mut self,
        strike_id: u32,
        total_damage: f32,
        objects_hit: u32,
        objects_destroyed: u32,
        wave_shell_count: u32,
        is_final_wave: bool,
        epicenters: &[Vec3],
    ) {
        let mut spawn_radiation: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        let mut spawn_toxin: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        let mut spawn_scud_poison: Vec<(ObjectId, super::Team, Vec3, u32, ScudStormAnthraxTier)> =
            Vec::new();
        let mut spawn_orbit: Option<(ObjectId, super::Team, Vec3, u32, SpectreGunshipScienceTier)> = None;
        let mut spawn_beam: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        if let Some(strike) = self.strikes.get_mut(&strike_id) {
            if strike.phase == HostStrikePhase::Queued {
                strike.total_damage_applied =
                    strike.total_damage_applied + total_damage;
                strike.objects_hit = strike.objects_hit.saturating_add(objects_hit);
                strike.objects_destroyed =
                    strike.objects_destroyed.saturating_add(objects_destroyed);
                strike.multi_strike_applied = strike
                    .multi_strike_applied
                    .saturating_add(wave_shell_count.max(1));
                // ScudStorm: per-missile LargePoisonField residual (each detonation).
                if strike.kind.spawns_scud_poison_field() {
                    let source = strike.source_object;
                    let team = strike.source_team;
                    let frame = strike.impact_frame;
                    let anthrax = strike.scud_anthrax_tier;
                    // PreAttack ends on first missile wave; FireFX + detonation residual.
                    if strike.scud_pre_attack_active {
                        strike.scud_pre_attack_active = false;
                    }
                    let shells = wave_shell_count.max(1);
                    strike.scud_fire_fx_applications =
                        strike.scud_fire_fx_applications.saturating_add(shells);
                    strike.scud_detonation_fx_applications =
                        strike.scud_detonation_fx_applications.saturating_add(shells);
                    strike.scud_launch_bone_applications =
                        strike.scud_launch_bone_applications.saturating_add(shells);
                    // ScudStormMissile loft residual (MissileAIUpdate / HeightDie /
                    // IgnitionFX / exhaust / SpecialPowerCompletionDie honesty).
                    strike.scud_missile_loft_applications =
                        strike.scud_missile_loft_applications.saturating_add(shells);
                    strike.scud_ignition_fx_applications =
                        strike.scud_ignition_fx_applications.saturating_add(shells);
                    strike.scud_launch_sound_applications =
                        strike.scud_launch_sound_applications.saturating_add(shells);
                    strike.scud_exhaust_applications =
                        strike.scud_exhaust_applications.saturating_add(shells);
                    strike.scud_height_die_applications =
                        strike.scud_height_die_applications.saturating_add(shells);
                    strike.scud_special_power_completion_applications = strike
                        .scud_special_power_completion_applications
                        .saturating_add(shells);
                    // PreferredHeight spring residual (Locomotor damping path).
                    // Host residual: spawn at PreferredHeight, spring sample, and
                    // loft phase honesty per missile wave. Fail-closed: not full
                    // live MissileAIUpdate physics / ThingFactory Object.
                    strike.scud_spawn_height_applications = strike
                        .scud_spawn_height_applications
                        .saturating_add(shells);
                    strike.scud_preferred_height_spring_applications = strike
                        .scud_preferred_height_spring_applications
                        .saturating_add(shells);
                    // Sample spring from ground (0) over HeightDie InitialDelay
                    // frames toward PreferredHeight (retail loft climb residual).
                    let spring_h = scud_missile_preferred_height_after_frames(
                        0.0,
                        SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES,
                    );
                    strike.scud_last_spring_height = spring_h;
                    // Ballistic flight residual: locomotor path toward first epicenter
                    // (or strike target) with OnlyWhenMovingDown / SnapToGround honesty.
                    let flight_target = epicenters
                        .first()
                        .copied()
                        .unwrap_or(strike.target_position);
                    // Launch residual near building; host uses target - offset as pad.
                    let launch = Vec3::new(
                        flight_target.x - SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING,
                        0.0,
                        flight_target.z,
                    );
                    // Sample enough frames to cover loft→turn→dive→HeightDie residual.
                    let sample_frames = ((SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING
                        + SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING)
                        / scud_missile_speed_per_frame())
                    .ceil() as u32
                        + SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES;
                    let (flight_pos, flight_dist, _dist_to, flight_phase) =
                        scud_missile_ballistic_sample(launch, flight_target, sample_frames);
                    strike.scud_ballistic_flight_applications = strike
                        .scud_ballistic_flight_applications
                        .saturating_add(shells);
                    strike.scud_only_moving_down_applications = strike
                        .scud_only_moving_down_applications
                        .saturating_add(shells);
                    strike.scud_snap_to_ground_applications = strike
                        .scud_snap_to_ground_applications
                        .saturating_add(shells);
                    strike.scud_model_draw_applications = strike
                        .scud_model_draw_applications
                        .saturating_add(shells);
                    strike.scud_last_flight_distance = flight_dist;
                    if flight_dist > strike.scud_peak_flight_distance {
                        strike.scud_peak_flight_distance = flight_dist;
                    }
                    // Pre-snap height residual lives in spring sample; snap sets Y=0.
                    strike.scud_last_flight_height = if flight_phase
                        == ScudMissileLoftPhase::HeightDie
                    {
                        0.0
                    } else {
                        flight_pos.y
                    };
                    // Peak loft phase residual: prefer ballistic sample, fall back.
                    let phase = if flight_phase.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8() {
                        flight_phase
                    } else {
                        scud_missile_loft_phase(
                            SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING + 1.0,
                            SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING * 0.5,
                            SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET * 0.5,
                        )
                    };
                    if phase.as_u8() > strike.scud_loft_phase_peak.as_u8() {
                        strike.scud_loft_phase_peak = phase;
                    }
                    if epicenters.is_empty() {
                        spawn_scud_poison.push((
                            source,
                            team,
                            strike.target_position,
                            frame,
                            anthrax,
                        ));
                    } else {
                        for p in epicenters {
                            spawn_scud_poison.push((source, team, *p, frame, anthrax));
                        }
                    }
                }
                if is_final_wave {
                    strike.phase = HostStrikePhase::Completed;
                    self.completed_this_frame.push(strike_id);
                    if strike.kind.spawns_radiation() {
                        spawn_radiation = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                        ));
                    }
                    // AnthraxBomb toxin (not Scud — Scud already spawned per-missile).
                    if strike.kind.spawns_toxin_field() && !strike.kind.spawns_scud_poison_field() {
                        spawn_toxin = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                        ));
                    }
                    if strike.kind.spawns_orbit_field() {
                        spawn_orbit = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                            strike.spectre_tier,
                        ));
                    }
                    if strike.kind.spawns_beam_field() {
                        // READY_TO_FIRE → FIRING residual on beam spawn.
                        apply_particle_charge_status(strike, strike.impact_frame);
                        if strike.particle_status != ParticleUplinkStatus::Firing {
                            // Force FIRING honesty when beam field is about to spawn.
                            let prev = strike.particle_status;
                            strike.particle_status = ParticleUplinkStatus::Firing;
                            if prev != ParticleUplinkStatus::Firing {
                                strike.particle_intensity_transitions =
                                    strike.particle_intensity_transitions.saturating_add(1);
                            }
                            if strike.particle_status_peak.as_u8()
                                < ParticleUplinkStatus::Firing.as_u8()
                            {
                                strike.particle_status_peak = ParticleUplinkStatus::Firing;
                            }
                            strike.particle_model_deployed_sets =
                                strike.particle_model_deployed_sets.saturating_add(1);
                        }
                        spawn_beam = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                        ));
                    }
                }
            }
        }
        if let Some((source, team, pos, impact_frame)) = spawn_radiation {
            self.spawn_radiation_field(source, team, pos, impact_frame, strike_id);
        }
        if let Some((source, team, pos, impact_frame)) = spawn_toxin {
            self.spawn_toxin_field(source, team, pos, impact_frame, strike_id);
        }
        for (source, team, pos, impact_frame, anthrax) in spawn_scud_poison {
            self.spawn_scud_poison_field_with_tier(
                source,
                team,
                pos,
                impact_frame,
                strike_id,
                anthrax,
            );
        }
        if let Some((source, team, pos, impact_frame, spectre_tier)) = spawn_orbit {
            self.spawn_orbit_field_with_tier(
                source,
                team,
                pos,
                impact_frame,
                strike_id,
                spectre_tier,
            );
        }
        if let Some((source, team, pos, impact_frame)) = spawn_beam {
            self.spawn_beam_field(source, team, pos, impact_frame, strike_id);
        }
    }

    /// Spawn a residual radiation field at `position` (NuclearMissile impact).
    pub fn spawn_radiation_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_radiation_id;
        self.next_radiation_id = self.next_radiation_id.saturating_add(1).max(1);
        let field = HostRadiationField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(NUKE_RADIATION_DURATION_FRAMES),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
        };
        self.radiation_fields.push(field);
        self.radiation_spawned_this_frame.push(id);
        self.radiation_fields_spawned_total =
            self.radiation_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build radiation damage plans for all fields whose tick frame has arrived.
    ///
    /// Retail `NukeRadiationFieldWeapon` hits ALLIES ENEMIES NEUTRALS (not
    /// airborne). Host residual damages all living objects in radius except
    /// the source launcher object. Fail-closed vs airborne filter / armor
    /// matrix / cleanup-hazard stacking.
    pub fn plan_due_radiation_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostRadiationTickPlan> {
        let mut plans = Vec::new();
        for field in &self.radiation_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= NUKE_RADIATION_RADIUS {
                    hits.push(HostRadiationDamageHit {
                        target_id: id,
                        damage: NUKE_RADIATION_DAMAGE_PER_TICK,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostRadiationTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record radiation tick results and advance next_tick_frame.
    pub fn record_radiation_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.radiation_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(NUKE_RADIATION_TICK_INTERVAL_FRAMES);
            self.radiation_damage_applications_total = self
                .radiation_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired radiation fields.
    pub fn prune_expired_radiation(&mut self, current_frame: u32) {
        self.radiation_fields
            .retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual toxin field at `position` (AnthraxBomb impact defaults).
    pub fn spawn_toxin_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_toxin_field_with_params(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            ANTHRAX_TOXIN_DAMAGE_PER_TICK,
            ANTHRAX_TOXIN_RADIUS,
            ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
            ANTHRAX_TOXIN_DURATION_FRAMES,
        )
    }

    /// Spawn residual LargePoisonField toxin (ScudStorm OCL_PoisonFieldLarge).
    pub fn spawn_scud_poison_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_scud_poison_field_with_tier(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            ScudStormAnthraxTier::Base,
        )
    }

    /// Spawn ScudStorm LargePoison residual with anthrax-upgrade tier stats.
    pub fn spawn_scud_poison_field_with_tier(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        anthrax_tier: ScudStormAnthraxTier,
    ) -> u32 {
        self.spawn_toxin_field_with_params(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            anthrax_tier.poison_damage_per_tick(),
            SCUD_STORM_POISON_RADIUS,
            SCUD_STORM_POISON_TICK_INTERVAL_FRAMES,
            SCUD_STORM_POISON_DURATION_FRAMES,
        )
    }

    /// Spawn a residual toxin field with explicit weapon residual params.
    pub fn spawn_toxin_field_with_params(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        damage_per_tick: f32,
        radius: f32,
        tick_interval_frames: u32,
        duration_frames: u32,
    ) -> u32 {
        let id = self.next_toxin_id;
        self.next_toxin_id = self.next_toxin_id.saturating_add(1).max(1);
        let field = HostToxinField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(duration_frames),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            damage_per_tick,
            radius,
            tick_interval_frames,
        };
        self.toxin_fields.push(field);
        self.toxin_spawned_this_frame.push(id);
        self.toxin_fields_spawned_total = self.toxin_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build toxin damage plans for all fields whose tick frame has arrived.
    ///
    /// Retail `AnthraxBombPoisonFieldWeapon` hits ALLIES ENEMIES NEUTRALS
    /// NOT_AIRBORNE. Host residual damages all living objects in radius except
    /// the source launcher object. Fail-closed vs airborne filter / armor
    /// matrix / cleanup-hazard stacking / gamma upgrade.
    pub fn plan_due_toxin_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostToxinTickPlan> {
        let mut plans = Vec::new();
        for field in &self.toxin_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= field.radius {
                    hits.push(HostToxinDamageHit {
                        target_id: id,
                        damage: field.damage_per_tick,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostToxinTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record toxin tick results and advance next_tick_frame.
    pub fn record_toxin_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.toxin_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(field.tick_interval_frames.max(1));
            self.toxin_damage_applications_total = self
                .toxin_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired toxin fields.
    pub fn prune_expired_toxin(&mut self, current_frame: u32) {
        self.toxin_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual Spectre orbit field at `position` (orbit insertion).
    /// Uses default Level2 OrbitTime (15s) when no tier is supplied.
    pub fn spawn_orbit_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_orbit_field_with_tier(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            SpectreGunshipScienceTier::Level2,
        )
    }

    /// Spawn Spectre orbit field with science-tier OrbitTime residual.
    pub fn spawn_orbit_field_with_tier(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        spectre_tier: SpectreGunshipScienceTier,
    ) -> u32 {
        let id = self.next_orbit_id;
        self.next_orbit_id = self.next_orbit_id.saturating_add(1).max(1);
        let duration = spectre_tier.orbit_duration_frames();
        let field = HostSpectreOrbitField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(duration),
            // First howitzer residual tick on orbit insertion frame.
            next_tick_frame: spawn_frame,
            // First gattling residual tick on orbit insertion frame.
            next_gattling_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            howitzer_ticks: 0,
            gattling_ticks: 0,
            gattling_consecutive: 0,
            howitzer_consecutive: 0,
            gattling_fire_level: 0,
            howitzer_fire_level: 0,
            gattling_coast_until_frame: 0,
            howitzer_coast_until_frame: 0,
            gattling_coast_applications: 0,
            howitzer_coast_applications: 0,
            rapid_fire_voice_cues: 0,
            model_condition_mean_sets: 0,
            model_condition_fast_sets: 0,
            model_condition_slow_sets: 0,
            howitzer_shells_spawned: 0,
            howitzer_shell_fire_fx: 0,
            howitzer_shell_detonation_fx: 0,
            howitzer_shell_height_die_delays: 0,
            howitzer_shell_fire_sounds: 0,
            howitzer_shell_dumb_projectile_applications: 0,
            howitzer_shell_physics_mass_applications: 0,
            howitzer_shell_death_detonated_applications: 0,
            howitzer_shell_death_lasered_applications: 0,
            howitzer_shell_only_moving_down_applications: 0,
            howitzer_shell_model_draw_applications: 0,
            howitzer_shell_scale_applications: 0,
            howitzer_shell_shadow_applications: 0,
            howitzer_shell_geometry_applications: 0,
            howitzer_shell_max_health_applications: 0,
        };
        self.orbit_fields.push(field);
        self.orbit_spawned_this_frame.push(id);
        self.orbit_fields_spawned_total = self.orbit_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build Spectre orbit damage plans for all fields whose tick frame has arrived.
    ///
    /// Wave 13 dual residual:
    /// - Howitzer (`SpectreHowitzerGun`): PrimaryDamage **80** in PrimaryDamageRadius
    ///   **25** around reticle + deterministic RandomOffsetForHowitzer residual.
    /// - Gattling (`SpectreGattlingGun`): PrimaryDamage **90** to nearest living
    ///   enemy in AttackAreaRadius **200** (single-target residual).
    /// Both exclude source launcher and same-team friendlies.
    /// Continuous-fire ROF residual advances on record_orbit_tick_complete.
    /// SpectreHowitzerShell projectile residual honesty is recorded on each
    /// howitzer tick (not full DumbProjectileBehavior Object / HeightDie flight).
    pub fn plan_due_orbit_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostSpectreOrbitTickPlan> {
        let mut plans = Vec::new();
        for field in &self.orbit_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let howitzer_due = field.is_due_howitzer(current_frame);
            let gattling_due = field.is_due_gattling(current_frame);
            // Accumulate damage per target (howitzer AOE + gattling single-target).
            let mut dmg_map: std::collections::BTreeMap<ObjectId, f32> =
                std::collections::BTreeMap::new();

            if howitzer_due {
                let off = spectre_howitzer_offset(field.howitzer_ticks);
                let epicenter = Vec3::new(
                    field.position.x + off.x,
                    field.position.y,
                    field.position.z + off.z,
                );
                for &(id, pos, team, alive) in object_positions {
                    if !alive || id == field.source_object || team == field.source_team {
                        continue;
                    }
                    let dist = horizontal_distance(pos, epicenter);
                    if dist <= SPECTRE_HOWITZER_RADIUS {
                        *dmg_map.entry(id).or_insert(0.0) += SPECTRE_ORBIT_DAMAGE_PER_TICK;
                    }
                }
            }

            if gattling_due {
                let mut best: Option<(ObjectId, f32)> = None;
                for &(id, pos, team, alive) in object_positions {
                    if !alive || id == field.source_object || team == field.source_team {
                        continue;
                    }
                    let dist = horizontal_distance(pos, field.position);
                    if dist <= SPECTRE_ORBIT_RADIUS {
                        match best {
                            Some((_, bd)) if bd <= dist => {}
                            _ => best = Some((id, dist)),
                        }
                    }
                }
                if let Some((id, _)) = best {
                    *dmg_map.entry(id).or_insert(0.0) += SPECTRE_GATTLING_DAMAGE;
                }
            }

            let hits: Vec<HostSpectreOrbitDamageHit> = dmg_map
                .into_iter()
                .filter(|(_, d)| *d > 0.0)
                .map(|(target_id, damage)| HostSpectreOrbitDamageHit {
                    target_id,
                    damage,
                    field_id: field.id,
                })
                .collect();
            plans.push(HostSpectreOrbitTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record Spectre orbit tick results and advance howitzer/gattling timers.
    pub fn record_orbit_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        // Apply ContinuousFireCoast cool-down before arming new shots this frame.
        self.apply_orbit_coast_cooldown(current_frame);
        if let Some(field) = self.orbit_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            // Advance whichever residual streams were due this frame.
            // Continuous-fire residual: consecutive shot counters raise ROF
            // (gattling 200%/300%, howitzer 150%/200%) after ContinuousFireOne/Two.
            // ContinuousFireCoast residual arms spin-down deadline after each shot.
            if current_frame >= field.next_tick_frame {
                field.howitzer_consecutive = field.howitzer_consecutive.saturating_add(1);
                let interval = spectre_howitzer_interval_frames(field.howitzer_consecutive);
                field.next_tick_frame = current_frame.saturating_add(interval);
                field.howitzer_ticks = field.howitzer_ticks.saturating_add(1);
                // SpectreHowitzerShell projectile residual honesty (not full Object).
                // Retail: ProjectileObject=SpectreHowitzerShell, FireFX, detonation
                // FX, FireSound, HeightDie InitialDelay pad-safe loft residual.
                field.howitzer_shells_spawned =
                    field.howitzer_shells_spawned.saturating_add(1);
                field.howitzer_shell_fire_fx =
                    field.howitzer_shell_fire_fx.saturating_add(1);
                field.howitzer_shell_detonation_fx =
                    field.howitzer_shell_detonation_fx.saturating_add(1);
                field.howitzer_shell_height_die_delays =
                    field.howitzer_shell_height_die_delays.saturating_add(1);
                field.howitzer_shell_fire_sounds =
                    field.howitzer_shell_fire_sounds.saturating_add(1);
                // DumbProjectileBehavior + Physics mass + InstantDeath + HeightDie
                // OnlyWhenMovingDown residual honesty (not full W3D shell Object).
                field.howitzer_shell_dumb_projectile_applications = field
                    .howitzer_shell_dumb_projectile_applications
                    .saturating_add(1);
                field.howitzer_shell_physics_mass_applications = field
                    .howitzer_shell_physics_mass_applications
                    .saturating_add(1);
                field.howitzer_shell_death_detonated_applications = field
                    .howitzer_shell_death_detonated_applications
                    .saturating_add(1);
                field.howitzer_shell_death_lasered_applications = field
                    .howitzer_shell_death_lasered_applications
                    .saturating_add(1);
                field.howitzer_shell_only_moving_down_applications = field
                    .howitzer_shell_only_moving_down_applications
                    .saturating_add(1);
                // W3D ModelDraw / Scale / Shadow / Geometry / MaxHealth residual
                // (fail-closed vs full ThingFactory Object / live Physics flight).
                field.howitzer_shell_model_draw_applications = field
                    .howitzer_shell_model_draw_applications
                    .saturating_add(1);
                field.howitzer_shell_scale_applications = field
                    .howitzer_shell_scale_applications
                    .saturating_add(1);
                field.howitzer_shell_shadow_applications = field
                    .howitzer_shell_shadow_applications
                    .saturating_add(1);
                field.howitzer_shell_geometry_applications = field
                    .howitzer_shell_geometry_applications
                    .saturating_add(1);
                field.howitzer_shell_max_health_applications = field
                    .howitzer_shell_max_health_applications
                    .saturating_add(1);
                field.howitzer_coast_until_frame =
                    spectre_coast_until_after_shot(current_frame, interval);
                let prev_level = field.howitzer_fire_level;
                if field.howitzer_consecutive > SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO {
                    field.howitzer_fire_level = 2;
                } else if field.howitzer_consecutive > SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE {
                    field.howitzer_fire_level = field.howitzer_fire_level.max(1);
                }
                // VoiceRapidFire residual when entering FAST (FiringTracker::speedUp).
                if prev_level < 2 && field.howitzer_fire_level == 2 {
                    field.rapid_fire_voice_cues = field.rapid_fire_voice_cues.saturating_add(1);
                }
                // MODELCONDITION_CONTINUOUS_FIRE_* residual (FiringTracker::speedUp).
                if prev_level < 1 && field.howitzer_fire_level >= 1 {
                    field.model_condition_mean_sets =
                        field.model_condition_mean_sets.saturating_add(1);
                }
                if prev_level < 2 && field.howitzer_fire_level == 2 {
                    field.model_condition_fast_sets =
                        field.model_condition_fast_sets.saturating_add(1);
                }
            }
            if current_frame >= field.next_gattling_tick_frame {
                field.gattling_consecutive = field.gattling_consecutive.saturating_add(1);
                let interval = spectre_gattling_interval_frames(field.gattling_consecutive);
                field.next_gattling_tick_frame = current_frame.saturating_add(interval);
                field.gattling_ticks = field.gattling_ticks.saturating_add(1);
                field.gattling_coast_until_frame =
                    spectre_coast_until_after_shot(current_frame, interval);
                let prev_level = field.gattling_fire_level;
                if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO {
                    field.gattling_fire_level = 2;
                } else if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE {
                    field.gattling_fire_level = field.gattling_fire_level.max(1);
                }
                // VoiceRapidFire residual when entering FAST (FiringTracker::speedUp).
                if prev_level < 2 && field.gattling_fire_level == 2 {
                    field.rapid_fire_voice_cues = field.rapid_fire_voice_cues.saturating_add(1);
                }
                // MODELCONDITION_CONTINUOUS_FIRE_* residual (FiringTracker::speedUp).
                if prev_level < 1 && field.gattling_fire_level >= 1 {
                    field.model_condition_mean_sets =
                        field.model_condition_mean_sets.saturating_add(1);
                }
                if prev_level < 2 && field.gattling_fire_level == 2 {
                    field.model_condition_fast_sets =
                        field.model_condition_fast_sets.saturating_add(1);
                }
            }
            self.orbit_damage_applications_total = self
                .orbit_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Apply FiringTracker ContinuousFireCoast residual to all orbit fields.
    ///
    /// Retail: after ContinuousFireCoast (2000 ms / 60 frames) without a shot past
    /// the next possible fire frame, coolDown() zeros consecutive shots and clears
    /// MEAN/FAST ROF bonuses. Host residual applies the same spin-down to both
    /// gattling and howitzer streams independently.
    pub fn apply_orbit_coast_cooldown(&mut self, current_frame: u32) {
        for field in &mut self.orbit_fields {
            if let Some((consec, level)) = spectre_coast_spin_down(
                current_frame,
                field.gattling_coast_until_frame,
                field.gattling_fire_level,
                field.gattling_consecutive,
            ) {
                // MODELCONDITION_CONTINUOUS_FIRE_SLOW residual on coolDown.
                if field.gattling_fire_level > 0 {
                    field.model_condition_slow_sets =
                        field.model_condition_slow_sets.saturating_add(1);
                }
                field.gattling_consecutive = consec;
                field.gattling_fire_level = level;
                field.gattling_coast_until_frame = 0;
                field.gattling_coast_applications =
                    field.gattling_coast_applications.saturating_add(1);
            }
            if let Some((consec, level)) = spectre_coast_spin_down(
                current_frame,
                field.howitzer_coast_until_frame,
                field.howitzer_fire_level,
                field.howitzer_consecutive,
            ) {
                if field.howitzer_fire_level > 0 {
                    field.model_condition_slow_sets =
                        field.model_condition_slow_sets.saturating_add(1);
                }
                field.howitzer_consecutive = consec;
                field.howitzer_fire_level = level;
                field.howitzer_coast_until_frame = 0;
                field.howitzer_coast_applications =
                    field.howitzer_coast_applications.saturating_add(1);
            }
        }
    }

    /// Residual honesty: at least one howitzer tick applied.
    pub fn honesty_howitzer_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.howitzer_ticks > 0)
            || self
                .orbit_fields
                .iter()
                .any(|f| f.damage_applications > 0 && f.howitzer_ticks > 0)
    }

    /// Residual honesty: at least one gattling strafe tick applied.
    pub fn honesty_gattling_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.gattling_ticks > 0)
    }

    /// Residual honesty: gattling continuous-fire ramp reached MEAN or FAST.
    pub fn honesty_gattling_continuous_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.gattling_fire_level >= 1)
    }

    /// Residual honesty: howitzer continuous-fire ramp reached MEAN or FAST.
    pub fn honesty_howitzer_continuous_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.howitzer_fire_level >= 1)
    }

    /// Residual honesty: ContinuousFireCoast cool-down applied at least once.
    pub fn honesty_continuous_fire_coast_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.gattling_coast_applications > 0 || f.howitzer_coast_applications > 0
        }) && SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES == 60
    }

    /// Residual honesty: VoiceRapidFire cue when ContinuousFire entered FAST.
    pub fn honesty_voice_rapid_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.rapid_fire_voice_cues > 0)
            && SPECTRE_VOICE_RAPID_FIRE_AUDIO.contains("Rapid")
    }

    /// Residual honesty: SpectreHowitzerShell projectile residual spawned.
    ///
    /// Fail-closed: not full DumbProjectileBehavior Object / HeightDie flight /
    /// live W3D GPU shell drawable / PhysicsBehavior mass path.
    pub fn honesty_howitzer_shell_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shells_spawned > 0
                && f.howitzer_shell_fire_fx >= f.howitzer_shells_spawned
                && f.howitzer_shell_detonation_fx >= f.howitzer_shells_spawned
                && f.howitzer_shell_height_die_delays >= f.howitzer_shells_spawned
                && f.howitzer_shell_fire_sounds >= f.howitzer_shells_spawned
                && f.howitzer_shell_dumb_projectile_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_physics_mass_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_death_detonated_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_only_moving_down_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_model_draw_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_scale_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_shadow_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_geometry_applications >= f.howitzer_shells_spawned
                && f.howitzer_shell_max_health_applications >= f.howitzer_shells_spawned
        }) && SPECTRE_HOWITZER_SHELL_OBJECT == "SpectreHowitzerShell"
            && SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
            && (SPECTRE_HOWITZER_WEAPON_SPEED - 999.0).abs() < 0.01
            && SPECTRE_HOWITZER_FIRE_FX.contains("TankGun")
            && SPECTRE_HOWITZER_DETONATION_FX.contains("SpectreHowitzer")
            && SPECTRE_HOWITZER_FIRE_SOUND.contains("Artillery")
            && (SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT - 1.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_MASS - 1.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN
            && SPECTRE_HOWITZER_SHELL_MODEL.contains("SpectreShell")
            && SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX.contains("NukeGLA")
            && (SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL
            && SPECTRE_HOWITZER_SHELL_SHADOW.contains("SHADOW_DECAL")
            && SPECTRE_HOWITZER_SHELL_GEOMETRY == "Cylinder"
    }

    /// Residual honesty: SpectreHowitzerShell DumbProjectileBehavior path residual.
    ///
    /// Fail-closed: not full ThingFactory Object / live W3D GPU ModelDraw / Physics.
    pub fn honesty_howitzer_shell_dumb_projectile_ok(&self) -> bool {
        self.honesty_howitzer_shell_ok()
            && self.orbit_fields.iter().any(|f| {
                f.howitzer_shell_dumb_projectile_applications > 0
                    && f.howitzer_shell_physics_mass_applications > 0
                    && f.howitzer_shell_death_detonated_applications > 0
                    && f.howitzer_shell_death_lasered_applications > 0
                    && f.howitzer_shell_only_moving_down_applications > 0
                    && f.howitzer_shell_model_draw_applications > 0
                    && f.howitzer_shell_scale_applications > 0
            })
            && (SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT - 4.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SPEED - 1111.0).abs() < 0.01
    }

    /// Residual honesty: SpectreHowitzerShell W3D ModelDraw residual.
    ///
    /// Tracks model AVSpectreShell1 + Scale/Shadow/Geometry/MaxHealth honesty
    /// per shell spawn. Fail-closed: not full W3D drawable Object / GPU mesh submit.
    pub fn honesty_howitzer_shell_model_draw_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.howitzer_shell_model_draw_applications > 0
                && f.howitzer_shell_scale_applications
                    >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_shadow_applications
                    >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_geometry_applications
                    >= f.howitzer_shell_model_draw_applications
                && f.howitzer_shell_max_health_applications
                    >= f.howitzer_shell_model_draw_applications
        }) && SPECTRE_HOWITZER_SHELL_MODEL == "AVSpectreShell1"
            && (SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01
            && SPECTRE_HOWITZER_SHELL_SHADOW == "SHADOW_DECAL"
            && SPECTRE_HOWITZER_SHELL_GEOMETRY == "Cylinder"
            && SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL
            && (SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01
            && (SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS - 4.0).abs() < 0.01
    }

    /// Residual honesty: MODELCONDITION_CONTINUOUS_FIRE_MEAN/FAST residual sets.
    ///
    /// Fail-closed: not full drawable animation state / W3D model condition matrix.
    pub fn honesty_model_condition_continuous_fire_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| {
            f.model_condition_mean_sets > 0 || f.model_condition_fast_sets > 0
        })
    }

    /// Residual honesty: MODELCONDITION_CONTINUOUS_FIRE_SLOW residual on coolDown.
    pub fn honesty_model_condition_slow_ok(&self) -> bool {
        self.orbit_fields.iter().any(|f| f.model_condition_slow_sets > 0)
    }

    /// Drop expired Spectre orbit fields.
    pub fn prune_expired_orbit(&mut self, current_frame: u32) {
        self.apply_orbit_coast_cooldown(current_frame);
        self.orbit_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual Particle Uplink continuous beam field at `position`.
    pub fn spawn_beam_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_beam_id;
        self.next_beam_id = self.next_beam_id.saturating_add(1).max(1);
        let field = HostParticleBeamField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            // Orbital death after TotalFiringTime + WidthGrow decay tail
            // (retail orbitalDeathFrame = orbitalDecayStart + widthGrowFrames).
            expires_frame: particle_death_frame(spawn_frame),
            // First damage pulse on beam-start frame (retail m_nextDamagePulseFrame = now).
            next_tick_frame: spawn_frame,
            pulses_made: 0,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            last_swath_position: position,
            max_swath_offset: 0.0,
            swath_applications: 0,
            // First scorch/reveal on beam-start frame (retail m_nextScorchMarkFrame = now).
            next_scorch_frame: spawn_frame,
            scorch_marks_made: 0,
            reveal_applications: 0,
            ground_hit_fx_applications: 0,
            peak_width_scalar: 0.0,
            last_damage_radius: 0.0,
            last_width_scalar: 0.0,
            trough_width_scalar: 1.0,
            decay_samples: 0,
            last_scorch_position: position,
            last_scorch_radius: 0.0,
            // Default swath mode (AI residual); human override via
            // set_beam_override_destination flips to manual driving residual.
            manual_target_mode: false,
            override_destination: position,
            current_target_position: position,
            last_driving_click_frame: 0,
            second_last_driving_click_frame: 0,
            last_drive_update_frame: spawn_frame,
            manual_drive_distance_total: 0.0,
            manual_drive_applications: 0,
            fast_drive_applications: 0,
            // STATUS_FIRING client residual: Intense outer nodes + connector
            // lasers + laser-base flare + ground-to-orbit orbital laser.
            // Fail-closed: not full bone extract / drawable ThingFactory lasers.
            outer_node_systems_created: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_lasers_created: PARTICLE_OUTER_EFFECT_NUM_BONES,
            laser_base_flare_created: 1,
            ground_to_orbit_laser_created: 1,
            status: ParticleUplinkStatus::Firing,
            outer_intensity: ParticleIntensity::Intense,
            connector_intensity: ParticleIntensity::Intense,
            laser_base_intensity: ParticleIntensity::Intense,
            // First BeamLaunchFX on STATUS_FIRING entry (retail m_nextLaunchFXFrame = 0).
            beam_launch_fx_applications: 1,
            next_launch_fx_frame: spawn_frame.saturating_add(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES),
            postfire_applications: 0,
            packing_applications: 0,
            intensity_transitions: 1, // Idle/Ready → Firing on spawn
            connector_flare_created: 1,
            peak_outer_beam_draw_width: 0.0,
            last_outer_beam_draw_width: 0.0,
            peak_retail_laser_radius: 0.0,
            last_retail_laser_radius: 0.0,
            peak_retail_damage_radius: 0.0,
            last_retail_damage_radius: 0.0,
            // Orbital laser W3DLaserDraw params + Intense connector OuterBeamWidth.
            orbital_laser_draw_params_armed: 1,
            connector_outer_beam_width_armed: 1,
            // Multi-beam NumBeams + TilingScalar residual armed at STATUS_FIRING.
            num_beams_armed: PARTICLE_ORBITAL_LASER_NUM_BEAMS,
            tiling_scalar_armed: 1,
            last_scroll_uv: 0.0,
            peak_abs_scroll_uv: 0.0,
            scroll_uv_samples: 0,
            // Soft-edge color residual armed (Inner/Outer color constants).
            soft_edge_samples: 0,
            peak_soft_edge_outer_width: 0.0,
            last_soft_edge_outer_width: 0.0,
            last_soft_edge_outer_alpha: 0.0,
            last_soft_edge_tile_factor: 0.0,
            soft_edge_color_armed: 1,
            // Outer-node bone layout residual (FX01..FX05 ring + connector).
            // Fail-closed: not full W3D bone-world extract.
            outer_node_bone_layout_applications: PARTICLE_OUTER_EFFECT_NUM_BONES,
            last_outer_node_bone_position: particle_outer_node_bone_position(position, 0),
            connector_bone_layout_applications: 1,
        };
        self.beam_fields.push(field);
        self.beam_spawned_this_frame.push(id);
        self.beam_fields_spawned_total = self.beam_fields_spawned_total.saturating_add(1);
        id
    }

    /// Apply `setSpecialPowerOverridableDestination` residual to a live beam.
    ///
    /// C++: sets `m_overrideTargetDestination`, arms `m_manualTargetMode`, and
    /// records double-click frames for ManualFastDrivingSpeed. Host residual
    /// seeds `current_target_position` from the last swath/click epicenter when
    /// first entering manual mode.
    pub fn set_beam_override_destination(
        &mut self,
        field_id: u32,
        destination: Vec3,
        current_frame: u32,
    ) -> bool {
        if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == field_id) {
            if field.is_expired(current_frame) {
                return false;
            }
            field.second_last_driving_click_frame = field.last_driving_click_frame;
            field.last_driving_click_frame = current_frame;
            field.override_destination = destination;
            if !field.manual_target_mode {
                // Entering manual: seed from last residual epicenter (swath or click).
                field.current_target_position = if field.swath_applications > 0 {
                    field.last_swath_position
                } else {
                    field.position
                };
                field.last_drive_update_frame = current_frame;
            }
            field.manual_target_mode = true;
            true
        } else {
            false
        }
    }

    /// Advance manual beam positions for all fields in manual-target mode.
    ///
    /// C++ update each frame: move `m_currentTargetPosition` toward override at
    /// ManualDrivingSpeed (or Fast) / LOGICFRAMES_PER_SECOND, clamping so the
    /// step never overshoots. Call once per logic frame before damage planning.
    pub fn advance_manual_beam_drive(&mut self, current_frame: u32) {
        for field in &mut self.beam_fields {
            if !field.manual_target_mode || field.is_expired(current_frame) {
                continue;
            }
            let last = field.last_drive_update_frame;
            if current_frame <= last {
                continue;
            }
            let frames = current_frame - last;
            let fast = particle_is_fast_drive(
                field.last_driving_click_frame,
                field.second_last_driving_click_frame,
            );
            let max_step = particle_manual_speed_per_frame(fast) * frames as f32;
            let dx = field.override_destination.x - field.current_target_position.x;
            let dz = field.override_destination.z - field.current_target_position.z;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist > 1e-4 {
                let step = max_step.min(dist);
                let scale = step / dist;
                field.current_target_position.x += dx * scale;
                field.current_target_position.z += dz * scale;
                // Host residual keeps Y at click height (terrain Z fail-closed).
                field.manual_drive_distance_total += step;
                field.manual_drive_applications =
                    field.manual_drive_applications.saturating_add(1);
                if fast {
                    field.fast_drive_applications =
                        field.fast_drive_applications.saturating_add(1);
                }
            }
            field.last_drive_update_frame = current_frame;
        }
    }

    /// Build Particle Uplink beam pulse plans for all fields whose tick frame
    /// has arrived.
    ///
    /// Retail damages all alive objects in beam radius (DamageRadiusScalar ×
    /// laser radius) at the SwathOfDeath or manual-drive epicenter. Host residual
    /// damages living objects in WidthGrow-scaled [`PARTICLE_BEAM_RADIUS`] around
    /// the residual epicenter, excluding the source launcher and same-team
    /// friendlies (host strike convention). Fail-closed vs full building→target
    /// rotation matrix / full GPU laser width matrix. WidthGrow damage-radius
    /// residual scales radius 0→full over grow, holds full through
    /// TotalFiringTime, then shrinks full→0 over decay ([`PARTICLE_WIDTH_GROW_FRAMES`]).
    /// Manual driving residual uses override destination when armed.
    /// DamagePulseRemnant trail residual spawns on each completed pulse
    /// ([`spawn_remnant_field`]).
    pub fn plan_due_beam_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostParticleBeamTickPlan> {
        let mut plans = Vec::new();
        for field in &self.beam_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            // SwathOfDeath or manual-drive residual epicenter.
            let epicenter = field.residual_epicenter(field.pulses_made);
            // WidthGrow residual: damage radius ramps with laser width scalar.
            let width_scalar = particle_width_scalar(field.spawn_frame, current_frame);
            let damage_radius = particle_beam_damage_radius(field.spawn_frame, current_frame);
            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                // Fail-closed residual: do not damage friendlies (same team).
                if team == field.source_team {
                    continue;
                }
                let dist = horizontal_distance(pos, epicenter);
                if dist <= damage_radius {
                    hits.push(HostParticleBeamDamageHit {
                        target_id: id,
                        damage: PARTICLE_BEAM_DAMAGE_PER_PULSE,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostParticleBeamTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: epicenter,
                hits,
                damage_radius,
                width_scalar,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record Particle Uplink beam pulse results and advance next_tick_frame.
    ///
    /// Also spawns a DamagePulseRemnant trail residual at the pulse swath
    /// epicenter (retail ParticleUplinkCannonTrailRemnant).
    pub fn record_beam_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        let mut spawn_remnant: Option<(ObjectId, super::Team, Vec3, u32, u32)> = None;
        if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == field_id) {
            // Epicenter residual honesty for the pulse that just applied.
            let epicenter = field.residual_epicenter(field.pulses_made);
            if field.manual_target_mode {
                // Manual mode: still record last epicenter; swath offset honesty
                // remains 0 (no S-curve while player is driving).
                field.last_swath_position = epicenter;
            } else {
                let offset = particle_swath_offset(field.pulses_made);
                let offset_len = (offset.x * offset.x + offset.z * offset.z).sqrt();
                field.last_swath_position = epicenter;
                if offset_len > field.max_swath_offset {
                    field.max_swath_offset = offset_len;
                }
                if offset_len > 0.01 {
                    field.swath_applications = field.swath_applications.saturating_add(1);
                }
            }

            // WidthGrow grow/hold/decay residual honesty for the pulse that just applied.
            field.sample_width_honesty(current_frame);
            let damage_radius = particle_beam_damage_radius(field.spawn_frame, current_frame);
            field.last_damage_radius = damage_radius;

            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.pulses_made = field.pulses_made.saturating_add(1);
            // Fractional nextFactor scheduling residual (C++ orbital lifetime).
            // Also never schedule in the past relative to current_frame.
            let scheduled = particle_next_pulse_frame(field.spawn_frame, field.pulses_made);
            field.next_tick_frame = scheduled.max(current_frame.saturating_add(1));
            self.beam_damage_applications_total = self
                .beam_damage_applications_total
                .saturating_add(applications);
            // DamagePulseRemnant residual at this pulse's swath epicenter.
            spawn_remnant = Some((
                field.source_object,
                field.source_team,
                epicenter,
                field.id,
                field.parent_strike_id,
            ));
        }
        if let Some((source, team, pos, beam_id, strike_id)) = spawn_remnant {
            self.spawn_remnant_field(source, team, pos, current_frame, beam_id, strike_id);
        }
    }

    /// Spawn a residual DamagePulseRemnant trail field at `position`.
    pub fn spawn_remnant_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_beam_id: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_remnant_id;
        self.next_remnant_id = self.next_remnant_id.saturating_add(1).max(1);
        let field = HostParticleRemnantField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(PARTICLE_REMNANT_DURATION_FRAMES),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_beam_id,
            parent_strike_id,
        };
        self.remnant_fields.push(field);
        self.remnant_spawned_this_frame.push(id);
        self.remnant_fields_spawned_total = self.remnant_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build remnant trail damage plans for all fields whose tick frame arrived.
    ///
    /// Retail RadiusDamageAffects ALLIES ENEMIES NEUTRALS — host residual damages
    /// all living objects in radius except the source launcher (same as toxin /
    /// poison field residual). Fail-closed vs full Object / ImmortalBody stack.
    pub fn plan_due_remnant_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostParticleRemnantTickPlan> {
        let mut plans = Vec::new();
        for field in &self.remnant_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= PARTICLE_REMNANT_RADIUS {
                    hits.push(HostParticleRemnantDamageHit {
                        target_id: id,
                        damage: PARTICLE_REMNANT_DAMAGE_PER_TICK,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostParticleRemnantTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record remnant trail tick results and advance next_tick_frame.
    pub fn record_remnant_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.remnant_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(PARTICLE_REMNANT_TICK_INTERVAL_FRAMES.max(1));
            self.remnant_damage_applications_total = self
                .remnant_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Sample WidthGrow grow/hold/decay honesty for all live beam fields.
    ///
    /// Call each logic frame so decay-tail residual is observed even when no
    /// damage pulses remain (retail LASERSTATUS_DECAYING after TotalFiringTime).
    pub fn sample_beam_width_honesty(&mut self, current_frame: u32) {
        for field in &mut self.beam_fields {
            if !field.is_expired(current_frame) {
                field.sample_width_honesty(current_frame);
            }
        }
    }

    /// Drop expired Particle Uplink beam fields (after WidthGrow decay death).
    pub fn prune_expired_beam(&mut self, current_frame: u32) {
        self.beam_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Drop expired DamagePulseRemnant trail fields.
    pub fn prune_expired_remnant(&mut self, current_frame: u32) {
        self.remnant_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// CleanupArea residual: remove radiation fields whose epicenter is within
    /// `radius` of `center` (AmbulanceCleanHazardWeapon / HAZARD_CLEANUP residual).
    /// Returns number of fields cleared.
    pub fn clear_radiation_fields_in_radius(&mut self, center: Vec3, radius: f32) -> u32 {
        let before = self.radiation_fields.len();
        self.radiation_fields
            .retain(|f| horizontal_distance(f.position, center) > radius);
        (before.saturating_sub(self.radiation_fields.len())) as u32
    }

    /// CleanupArea residual: remove toxin fields whose epicenter is within
    /// `radius` of `center`. Returns number of fields cleared.
    pub fn clear_toxin_fields_in_radius(&mut self, center: Vec3, radius: f32) -> u32 {
        let before = self.toxin_fields.len();
        self.toxin_fields
            .retain(|f| horizontal_distance(f.position, center) > radius);
        (before.saturating_sub(self.toxin_fields.len())) as u32
    }

    /// Cancel pending strikes owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for strike in self.strikes.values_mut() {
            if strike.source_object == source && strike.phase == HostStrikePhase::Queued {
                strike.phase = HostStrikePhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// True if at least one strike of `kind` is currently queued.
    pub fn honesty_queue_ok(&self, kind: HostSuperweaponKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one strike of `kind` completed with damage applied
    /// (or completed cleanly with zero victims in radius — still "completed").
    pub fn honesty_complete_ok(&self, kind: HostSuperweaponKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|s| s.phase == HostStrikePhase::Completed)
    }

    /// True if at least one residual radiation field was spawned this session.
    pub fn honesty_radiation_ok(&self) -> bool {
        self.radiation_fields_spawned_total > 0
            || !self.radiation_fields.is_empty()
            || !self.radiation_spawned_this_frame.is_empty()
    }

    /// Stronger radiation honesty: residual field applied at least one damage tick.
    pub fn honesty_radiation_damage_ok(&self) -> bool {
        self.radiation_damage_applications_total > 0
            || self
                .radiation_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual toxin field was spawned this session.
    pub fn honesty_toxin_ok(&self) -> bool {
        self.toxin_fields_spawned_total > 0
            || !self.toxin_fields.is_empty()
            || !self.toxin_spawned_this_frame.is_empty()
    }

    /// Stronger toxin honesty: residual field applied at least one damage tick.
    pub fn honesty_toxin_damage_ok(&self) -> bool {
        self.toxin_damage_applications_total > 0
            || self
                .toxin_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual Spectre orbit field was spawned this session.
    pub fn honesty_orbit_ok(&self) -> bool {
        self.orbit_fields_spawned_total > 0
            || !self.orbit_fields.is_empty()
            || !self.orbit_spawned_this_frame.is_empty()
    }

    /// Stronger orbit honesty: residual field applied at least one damage tick.
    pub fn honesty_orbit_damage_ok(&self) -> bool {
        self.orbit_damage_applications_total > 0
            || self
                .orbit_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// True if at least one residual Particle Uplink beam field was spawned.
    pub fn honesty_beam_ok(&self) -> bool {
        self.beam_fields_spawned_total > 0
            || !self.beam_fields.is_empty()
            || !self.beam_spawned_this_frame.is_empty()
    }

    /// Stronger beam honesty: residual field applied at least one damage pulse.
    pub fn honesty_beam_damage_ok(&self) -> bool {
        self.beam_damage_applications_total > 0
            || self
                .beam_fields
                .iter()
                .any(|f| f.damage_applications > 0 || f.total_damage_applied > 0.0)
    }

    /// Residual honesty: SwathOfDeath epicenter walked off the click point.
    pub fn honesty_beam_swath_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.swath_applications > 0)
            || self
                .beam_fields
                .iter()
                .any(|f| f.max_swath_offset > 0.01)
    }

    /// Residual honesty: DamagePulseRemnant trail residual spawned from beam pulses.
    pub fn honesty_beam_remnant_ok(&self) -> bool {
        self.remnant_fields_spawned_total > 0
            || !self.remnant_fields.is_empty()
    }

    /// Residual honesty: remnant trail applied damage at least once.
    pub fn honesty_beam_remnant_damage_ok(&self) -> bool {
        self.remnant_damage_applications_total > 0
            || self
                .remnant_fields
                .iter()
                .any(|f| f.damage_applications > 0)
    }

    /// Residual honesty: WidthGrow damage-radius residual ramped past a floor.
    ///
    /// True when any beam field reached width scalar ≥ 0.5 (half WidthGrowTime).
    /// Fail-closed: not full GPU laser width matrix / OuterBeamWidth × scalar.
    pub fn honesty_beam_width_grow_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.peak_width_scalar >= 0.5)
            && PARTICLE_WIDTH_GROW_FRAMES == 60
    }

    /// Residual honesty: WidthGrow decay shrink residual after TotalFiringTime.
    ///
    /// True when any beam field sampled decay (trough scalar ≤ 0.5 after a
    /// full peak). Fail-closed: not full OuterBeamWidth GPU laser / drawable
    /// destroy after orbitalDeathFrame client path.
    pub fn honesty_beam_width_decay_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.decay_samples > 0
                && f.trough_width_scalar <= 0.5 + f32::EPSILON
                && f.peak_width_scalar >= 0.99
        }) && PARTICLE_WIDTH_GROW_FRAMES == 60
            && PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES
                == PARTICLE_BEAM_DURATION_FRAMES + PARTICLE_WIDTH_GROW_FRAMES
    }

    /// Residual honesty: multi-beam NumBeams + ScrollRate / TilingScalar residual.
    ///
    /// Tracks W3DLaserDraw NumBeams **12**, ScrollRate UV accumulation, and
    /// TilingScalar honesty on a live beam field. Fail-closed: not full GPU
    /// multi-beam soft edge / texture atlas submit (soft-edge residual closed
    /// separately via [`honesty_beam_soft_edge_ok`]).
    pub fn honesty_beam_num_beams_scroll_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.num_beams_armed == PARTICLE_ORBITAL_LASER_NUM_BEAMS
                && f.tiling_scalar_armed >= 1
                && f.scroll_uv_samples >= 1
                && f.peak_abs_scroll_uv > 0.0
        }) && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (PARTICLE_ORBITAL_LASER_SCROLL_RATE + 1.75).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_TILING_SCALAR - 0.15).abs() < 0.01
            && particle_orbital_laser_num_beams() == 12
            && (particle_orbital_laser_tiling_scalar() - 0.15).abs() < 0.01
            // 30 frames at ScrollRate -1.75 → UV = -1.75
            && (particle_orbital_laser_scroll_uv(0, 30) + 1.75).abs() < 0.01
    }

    /// Residual honesty: multi-beam soft-edge width/alpha/color/tile residual.
    ///
    /// Tracks W3DLaserDraw cylinder soft edge (`scale = i/(NumBeams-1)`),
    /// InnerColor/OuterColor lerp, and tile-factor honesty. Fail-closed: not
    /// full SegLineRenderer GPU texture atlas submit / live surface aspect.
    pub fn honesty_beam_soft_edge_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.soft_edge_color_armed >= 1
                && f.soft_edge_samples >= 1
                && f.peak_soft_edge_outer_width > 0.0
                && f.last_soft_edge_outer_alpha > 0.0
                && f.last_soft_edge_tile_factor > 0.0
        }) && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (particle_orbital_soft_edge_scale(0) - 0.0).abs() < 0.01
            && (particle_orbital_soft_edge_scale(11) - 1.0).abs() < 0.01
            && (particle_orbital_soft_edge_outer_width_peak() - 26.0).abs() < 0.01
            && (particle_orbital_soft_edge_alpha(0) - PARTICLE_ORBITAL_LASER_INNER_COLOR.3).abs()
                < 0.01
            && (particle_orbital_soft_edge_alpha(11) - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs()
                < 0.01
            && PARTICLE_ORBITAL_LASER_TILE
            && PARTICLE_ORBITAL_LASER_TEXTURE.contains("EXNoise")
            && (PARTICLE_ORBITAL_LASER_INNER_COLOR.0 - 1.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_OUTER_COLOR.2 - 1.0).abs() < 0.01
    }

    /// Residual honesty: OuterBeamWidth × width_scalar orbital laser residual.
    ///
    /// Tracks W3DLaserDraw OuterBeamWidth draw width, `getCurrentLaserRadius`
    /// (OuterBeamWidth×0.5×scalar), and retail damage formula
    /// (laser radius × DamageRadiusScalar = peak 44.2). Host combat damage
    /// still uses [`PARTICLE_BEAM_RADIUS`] (50). Fail-closed: not full GPU
    /// multi-beam soft edge / texture atlas submit (NumBeams residual closed
    /// separately via [`honesty_beam_num_beams_scroll_ok`]).
    pub fn honesty_beam_outer_beam_width_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.orbital_laser_draw_params_armed >= 1
                && f.connector_outer_beam_width_armed >= 1
                && f.peak_outer_beam_draw_width
                    >= PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH * 0.5 - f32::EPSILON
                && f.peak_retail_laser_radius
                    >= particle_orbital_laser_template_width() * 0.5 - f32::EPSILON
                && f.peak_retail_damage_radius
                    >= particle_orbital_laser_template_width()
                        * 0.5
                        * PARTICLE_DAMAGE_RADIUS_SCALAR
                        - 0.1
        }) && (PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - 26.0).abs() < 0.01
            && (PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH - 0.6).abs() < 0.01
            && PARTICLE_ORBITAL_LASER_NUM_BEAMS == 12
            && (PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH - 2.0).abs() < 0.01
            && (PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH - 1.2).abs() < 0.01
            && PARTICLE_ORBITAL_LASER_TEXTURE.contains("EXNoise")
            && (particle_orbital_laser_template_width() - 13.0).abs() < 0.01
            && (particle_retail_damage_radius(0, PARTICLE_WIDTH_GROW_FRAMES) - 44.2).abs() < 0.05
    }

    /// Residual honesty: manual beam drive moved the epicenter at least once.
    ///
    /// Fail-closed: not full scripted waypoint mode / disabled-object reject /
    /// terrain height snap on every frame.
    pub fn honesty_beam_manual_drive_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.manual_target_mode && f.manual_drive_distance_total > 0.01
        }) && (PARTICLE_MANUAL_DRIVING_SPEED - 20.0).abs() < 0.01
    }

    /// Residual honesty: ManualFastDrivingSpeed used after double-click residual.
    pub fn honesty_beam_fast_drive_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.fast_drive_applications > 0)
            && (PARTICLE_MANUAL_FAST_DRIVING_SPEED - 40.0).abs() < 0.01
            && PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES == 15
    }

    /// Residual honesty: STATUS_FIRING outer-node + connector laser residual.
    ///
    /// Fail-closed: not full W3D bone-world convert / live LaserUpdate drawable
    /// matrix (bone layout residual closed via
    /// [`honesty_beam_outer_node_bone_layout_ok`]; intensity schedule residual
    /// closed separately via [`honesty_beam_intensity_schedule_ok`]).
    pub fn honesty_beam_outer_nodes_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.outer_node_systems_created == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.connector_lasers_created == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.laser_base_flare_created >= 1
                && f.ground_to_orbit_laser_created >= 1
        }) && PARTICLE_OUTER_EFFECT_NUM_BONES == 5
            && PARTICLE_OUTER_NODE_INTENSE_FLARE.contains("Intense")
            && PARTICLE_CONNECTOR_INTENSE_LASER.contains("Intense")
            && PARTICLE_ORBITAL_LASER_NAME.contains("OrbitalLaser")
    }

    /// Residual honesty: outer-node FX01..FX05 bone layout + connector residual.
    ///
    /// Host residual places bones on a ring around the building origin
    /// (fail-closed vs full W3D bone-world matrix extract / dish mesh attach).
    pub fn honesty_beam_outer_node_bone_layout_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.outer_node_bone_layout_applications == PARTICLE_OUTER_EFFECT_NUM_BONES
                && f.connector_bone_layout_applications >= 1
                && f.last_outer_node_bone_position != Vec3::ZERO
        }) && PARTICLE_OUTER_EFFECT_NUM_BONES == 5
            && particle_outer_node_bone_name(0) == "FX01"
            && particle_outer_node_bone_name(4) == "FX05"
            && PARTICLE_CONNECTOR_BONE_NAME == "FXConnector"
            && PARTICLE_FIRE_BONE_NAME == "FXMain"
            && (PARTICLE_OUTER_NODE_RING_RADIUS - 40.0).abs() < 0.01
            && (PARTICLE_OUTER_NODE_RING_HEIGHT - 25.0).abs() < 0.01
            && PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS == 5
            && PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS == 4
            && PARTICLE_CONNECTOR_LASER_TEXTURE.contains("EXLaser")
    }

    /// Residual honesty: CHARGING/PREPARING/ALMOST_READY/READY intensity schedule.
    ///
    /// True when a ParticleCannon strike observed at least PREPARING residual
    /// (or ALMOST_READY when impact_delay only covers the late window) and
    /// BeamLaunchFX / POSTFIRE intensity residual exists on a beam field.
    /// Fail-closed: not full W3D bone extract / live ParticleSystem manager.
    pub fn honesty_beam_intensity_schedule_ok(&self) -> bool {
        // Pre-fire residual: host impact_delay (120) only covers PREPARING→
        // ALMOST_READY→READY (full CHARGING needs BeginCharge+RaiseAntenna
        // windows that exceed impact_delay).
        let strike_ok = self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ParticleCannon
                && s.particle_intensity_transitions >= 1
                && (s.particle_preparing_applications > 0
                    || s.particle_almost_ready_applications > 0
                    || s.particle_ready_applications > 0
                    || s.particle_charging_applications > 0)
                && s.particle_status_peak.as_u8()
                    >= ParticleUplinkStatus::Preparing.as_u8()
        });
        let beam_ok = self.beam_fields.iter().any(|f| {
            f.beam_launch_fx_applications >= 1
                && f.outer_intensity == ParticleIntensity::Intense
                && PARTICLE_LAUNCH_FX_INTERVAL_FRAMES == 30
                && PARTICLE_BEAM_LAUNCH_FX.contains("BeamLaunch")
        });
        let timing_ok = PARTICLE_BEGIN_CHARGE_FRAMES == 150
            && PARTICLE_RAISE_ANTENNA_FRAMES == 140
            && PARTICLE_READY_DELAY_FRAMES == 60
            && PARTICLE_BEAM_TRAVEL_FRAMES == 75;
        (strike_ok || beam_ok) && timing_ok
    }

    /// Residual honesty: POSTFIRE Medium intensity after TotalFiringTime.
    pub fn honesty_beam_postfire_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| {
            f.postfire_applications > 0
                && (f.status == ParticleUplinkStatus::Postfire
                    || f.status == ParticleUplinkStatus::Packing
                    || f.outer_intensity == ParticleIntensity::Medium)
        })
    }

    /// Residual honesty: BeamLaunchFX refresh residual while STATUS_FIRING.
    pub fn honesty_beam_launch_fx_ok(&self) -> bool {
        self.beam_fields
            .iter()
            .any(|f| f.beam_launch_fx_applications >= 2)
            && PARTICLE_LAUNCH_FX_INTERVAL_FRAMES == 30
    }

    /// Residual honesty: ScudStorm PreAttack + Chem FXBone residual.
    ///
    /// Fail-closed: not full ScudStormMissile ThingFactory Object path.
    pub fn honesty_scud_pre_attack_and_chem_fx_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_chem_fx_bones == SCUD_STORM_CHEM_FX_BONE_COUNT
                && s.scud_launch_bone_applications >= 1
                && (s.scud_pre_attack_frames > 0
                    || s.scud_fire_fx_applications > 0
                    || s.scud_pre_attack_active)
        }) && SCUD_STORM_CHEM_FX_BONE_COUNT == 3
            && SCUD_STORM_CHEM_FX_PARTICLE.contains("Goo")
            && SCUD_STORM_FIRE_FX.contains("ScudStormMissile")
            && SCUD_STORM_LAUNCH_BONE == "WeaponA"
    }

    /// Residual honesty: ScudStormMissile MissileAIUpdate loft residual.
    ///
    /// Tracks loft / IgnitionFX / FireSound / exhaust / HeightDie /
    /// SpecialPowerCompletionDie residual per missile wave. Fail-closed: not
    /// full ThingFactory projectile Object / live MissileAIUpdate physics sim
    /// (PreferredHeight spring residual closed separately via
    /// [`honesty_scud_preferred_height_spring_ok`]).
    pub fn honesty_scud_missile_loft_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_missile_loft_applications > 0
                && s.scud_ignition_fx_applications >= s.scud_missile_loft_applications
                && s.scud_launch_sound_applications >= s.scud_missile_loft_applications
                && s.scud_exhaust_applications >= s.scud_missile_loft_applications
                && s.scud_height_die_applications >= s.scud_missile_loft_applications
                && s.scud_special_power_completion_applications
                    >= s.scud_missile_loft_applications
                && s.scud_fire_fx_applications >= s.scud_missile_loft_applications
        }) && SCUD_STORM_MISSILE_OBJECT == "ScudStormMissile"
            && !SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET
            && SCUD_STORM_MISSILE_FUEL_LIFETIME == 0
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING - 500.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING - 200.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET - 15.0).abs() < 0.01
            && SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES == 30
            && (SCUD_STORM_MISSILE_PREFERRED_HEIGHT - 240.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01
            && SCUD_STORM_MISSILE_IGNITION_FX.contains("Ignition")
            && SCUD_STORM_MISSILE_LAUNCH_SOUND.contains("Launch")
            && SCUD_STORM_MISSILE_EXHAUST.contains("Exhaust")
            && SCUD_STORM_MISSILE_SPECIAL_POWER.contains("ScudStorm")
    }

    /// Residual honesty: ScudStormMissile PreferredHeight spring residual.
    ///
    /// Tracks spawn-at-PreferredHeight, Locomotor damping spring samples, and
    /// loft phase peak (Loft→Turn→Dive→HeightDie). Fail-closed: not full
    /// ThingFactory Object / live physics flight path (ballistic residual closed
    /// separately via [`honesty_scud_ballistic_flight_ok`]).
    pub fn honesty_scud_preferred_height_spring_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_spawn_height_applications > 0
                && s.scud_preferred_height_spring_applications
                    >= s.scud_spawn_height_applications
                && s.scud_loft_phase_peak.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8()
                && s.scud_last_spring_height > 0.0
        }) && (scud_missile_spawn_height() - 240.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_PREFERRED_HEIGHT_DAMPING - 0.7).abs() < 0.01
            // One spring step from ground: 0 + (240-0)*0.7 = 168.
            && (scud_missile_preferred_height_spring(0.0) - 168.0).abs() < 0.01
            // Already at preferred: spring holds height.
            && (scud_missile_preferred_height_spring(240.0) - 240.0).abs() < 0.01
            && scud_missile_loft_phase(0.0, 1000.0, 100.0) == ScudMissileLoftPhase::Loft
            && scud_missile_loft_phase(500.0, 1000.0, 200.0) == ScudMissileLoftPhase::Turn
            && scud_missile_loft_phase(600.0, 100.0, 100.0) == ScudMissileLoftPhase::Dive
            && scud_missile_loft_phase(600.0, 50.0, 10.0) == ScudMissileLoftPhase::HeightDie
    }

    /// Residual honesty: ScudStormMissile ballistic flight residual.
    ///
    /// Tracks locomotor speed/accel path sampling, OnlyWhenMovingDown,
    /// SnapToGroundOnDeath, and W3D model residual. Fail-closed: not full
    /// ThingFactory Object / live Physics motive force / turn-rate matrix.
    pub fn honesty_scud_ballistic_flight_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind == HostSuperweaponKind::ScudStorm
                && s.scud_ballistic_flight_applications > 0
                && s.scud_only_moving_down_applications
                    >= s.scud_ballistic_flight_applications
                && s.scud_snap_to_ground_applications
                    >= s.scud_ballistic_flight_applications
                && s.scud_model_draw_applications >= s.scud_ballistic_flight_applications
                && s.scud_peak_flight_distance > 0.0
                && s.scud_loft_phase_peak.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8()
        }) && SCUD_STORM_MISSILE_MODEL == "UBScudStrm_M"
            && SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN
            && SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH
            && SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_ACCEL - 675.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_SPEED_DAMAGED - 200.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MIN_SPEED - 100.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_TURN_RATE - 540.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_LOCOMOTOR_MAX_THRUST_ANGLE - 45.0).abs() < 0.01
            && (SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01
            && SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL
            && (scud_missile_speed_per_frame() - 10.0).abs() < 0.01
    }

    /// Residual honesty: once-at-queue multi-strike OCL residual plan.
    ///
    /// True when a multi-strike Artillery/Carpet/Scud strike stored epicenters
    /// + shell frames at queue (retail once-at-create stream residual).
    /// Fail-closed: not live mid-sim global stream mutation / full transport Object.
    pub fn honesty_once_at_queue_ocl_ok(&self) -> bool {
        self.strikes.values().any(|s| {
            s.kind.is_multi_strike()
                && s.ocl_once_at_queue_armed >= 1
                && !s.ocl_points.is_empty()
                && s.ocl_shell_frames.len() == s.ocl_points.len()
                && s.ocl_shell_frames.first().copied().unwrap_or(0) >= s.impact_frame
        })
    }

    /// Advance ParticleCannon pre-fire intensity schedule + beam FIRING/POSTFIRE/
    /// PACKING intensity residual + BeamLaunchFX refresh + Scud PreAttack residual.
    ///
    /// Call once per logic frame (before impact planning is fine).
    pub fn advance_particle_intensity_schedule(&mut self, current_frame: u32) {
        // Pre-fire charge residual on queued ParticleCannon strikes.
        let particle_ids: Vec<u32> = self
            .strikes
            .values()
            .filter(|s| {
                s.kind == HostSuperweaponKind::ParticleCannon
                    && s.phase == HostStrikePhase::Queued
            })
            .map(|s| s.id)
            .collect();
        for id in particle_ids {
            if let Some(strike) = self.strikes.get_mut(&id) {
                apply_particle_charge_status(strike, current_frame);
            }
        }

        // ScudStorm PreAttack residual frame counter (until first missile wave).
        for strike in self.strikes.values_mut() {
            if strike.kind == HostSuperweaponKind::ScudStorm
                && strike.phase == HostStrikePhase::Queued
                && strike.scud_pre_attack_active
                && current_frame >= strike.activate_frame
                && current_frame < strike.impact_frame
            {
                strike.scud_pre_attack_frames =
                    strike.scud_pre_attack_frames.saturating_add(1);
            }
        }

        // Beam attack-phase intensity residual (FIRING → POSTFIRE → PACKING).
        for field in &mut self.beam_fields {
            if field.is_expired(current_frame)
                && field.status != ParticleUplinkStatus::Packing
            {
                // Past orbital death: PACKING residual (effects cleared).
                if field.status != ParticleUplinkStatus::Packing {
                    field.intensity_transitions =
                        field.intensity_transitions.saturating_add(1);
                }
                field.status = ParticleUplinkStatus::Packing;
                field.packing_applications =
                    field.packing_applications.saturating_add(1);
                field.outer_intensity = ParticleIntensity::None;
                field.connector_intensity = ParticleIntensity::None;
                field.laser_base_intensity = ParticleIntensity::None;
                field.outer_node_systems_created = 0;
                field.connector_lasers_created = 0;
                field.laser_base_flare_created = 0;
                field.ground_to_orbit_laser_created = 0;
                field.connector_flare_created = 0;
                continue;
            }
            if field.is_expired(current_frame) {
                continue;
            }
            let next_status = particle_status_for_attack(
                current_frame,
                field.spawn_frame,
                PARTICLE_BEAM_DURATION_FRAMES,
                PARTICLE_WIDTH_GROW_FRAMES,
            );
            if next_status != field.status {
                field.intensity_transitions =
                    field.intensity_transitions.saturating_add(1);
                field.status = next_status;
                let fx = particle_client_effects_for_status(next_status);
                field.outer_node_systems_created = fx.outer_nodes;
                field.outer_intensity = fx.outer_intensity;
                field.connector_lasers_created = fx.connector_lasers;
                field.connector_intensity = fx.connector_intensity;
                field.connector_flare_created = fx.connector_flare;
                field.laser_base_flare_created = fx.laser_base;
                field.laser_base_intensity = fx.laser_base_intensity;
                field.ground_to_orbit_laser_created = fx.ground_to_orbit;
                match next_status {
                    ParticleUplinkStatus::Postfire => {
                        field.postfire_applications =
                            field.postfire_applications.saturating_add(1);
                    }
                    ParticleUplinkStatus::Packing => {
                        field.packing_applications =
                            field.packing_applications.saturating_add(1);
                    }
                    _ => {}
                }
            }
            // BeamLaunchFX residual refresh while STATUS_FIRING.
            if field.status == ParticleUplinkStatus::Firing
                && current_frame >= field.next_launch_fx_frame
            {
                field.beam_launch_fx_applications =
                    field.beam_launch_fx_applications.saturating_add(1);
                field.next_launch_fx_frame = current_frame
                    .saturating_add(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES)
                    .max(field.next_launch_fx_frame.saturating_add(1));
            }
        }
    }

    /// Residual honesty: TotalScorchMarks residual applied at least one mark.
    pub fn honesty_beam_scorch_ok(&self) -> bool {
        (self.beam_fields.iter().any(|f| f.scorch_marks_made > 0)
            || self
                .beam_fields
                .iter()
                .any(|f| f.ground_hit_fx_applications > 0))
            && PARTICLE_TOTAL_SCORCH_MARKS == 20
            && PARTICLE_GROUND_HIT_FX.contains("BeamHitsGround")
    }

    /// Residual honesty: RevealRange residual applied at least once with scorch.
    pub fn honesty_beam_reveal_ok(&self) -> bool {
        self.beam_fields.iter().any(|f| f.reveal_applications > 0)
            && (PARTICLE_REVEAL_RANGE - 50.0).abs() < 0.01
    }

    /// Apply due TotalScorchMarks / GroundHitFX / RevealRange residual events.
    ///
    /// Retail (STATUS_FIRING): when `m_nextScorchMarkFrame <= now`, spawn scorch,
    /// run GroundHitFX, and doShroudReveal/undoShroudReveal at current target with
    /// RevealRange. Host residual records honesty counters + last scorch position
    /// (fail-closed vs full TheGameClient::addScorch GPU / partition shroud cells
    /// without a wired ShroudManager hook from this registry).
    pub fn apply_due_beam_scorch_reveals(
        &mut self,
        current_frame: u32,
    ) -> Vec<HostParticleScorchRevealEvent> {
        let mut events = Vec::new();
        for field in &mut self.beam_fields {
            // Catch up all due scorch marks (may be multi if frames skipped).
            while field.is_due_scorch(current_frame) {
                let pulse_idx = particle_scorch_pulse_index(field.scorch_marks_made);
                let epicenter = field.residual_epicenter(pulse_idx);
                let scorch_r = particle_scorch_radius(field.spawn_frame, current_frame);
                field.scorch_marks_made = field.scorch_marks_made.saturating_add(1);
                field.ground_hit_fx_applications =
                    field.ground_hit_fx_applications.saturating_add(1);
                field.reveal_applications = field.reveal_applications.saturating_add(1);
                field.last_scorch_position = epicenter;
                field.last_scorch_radius = scorch_r;
                let scheduled =
                    particle_next_scorch_frame(field.spawn_frame, field.scorch_marks_made);
                // Advance by schedule factor; allow multi-mark catch-up when
                // frames were skipped (do not clamp to current+1 inside the loop).
                field.next_scorch_frame =
                    scheduled.max(field.next_scorch_frame.saturating_add(1));
                events.push(HostParticleScorchRevealEvent {
                    field_id: field.id,
                    source_object: field.source_object,
                    source_team: field.source_team,
                    position: epicenter,
                    scorch_radius: scorch_r,
                    reveal_range: PARTICLE_REVEAL_RANGE,
                    scorch_mark_index: field.scorch_marks_made,
                });
            }
        }
        events.sort_by_key(|e| (e.field_id, e.scorch_mark_index));
        events
    }

    /// Combined host path honesty: a completed strike exists for `kind`.
    /// NuclearMissile also requires residual radiation field spawn.
    /// AnthraxBomb also requires residual toxin field spawn.
    /// SpectreGunship also requires residual orbit field spawn.
    /// ParticleCannon also requires residual continuous beam field spawn.
    pub fn honesty_host_path_ok(&self, kind: HostSuperweaponKind) -> bool {
        if !self.honesty_complete_ok(kind) {
            return false;
        }
        if kind == HostSuperweaponKind::NuclearMissile {
            return self.honesty_radiation_ok();
        }
        if kind == HostSuperweaponKind::AnthraxBomb {
            return self.honesty_toxin_ok();
        }
        if kind == HostSuperweaponKind::SpectreGunship {
            return self.honesty_orbit_ok();
        }
        if kind == HostSuperweaponKind::ParticleCannon {
            return self.honesty_beam_ok();
        }
        true
    }
}

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn daisy_cutter_maps_from_command_powers() {
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::DaisyCutter),
            Some(HostSuperweaponKind::DaisyCutter)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::FuelAirBomb),
            Some(HostSuperweaponKind::DaisyCutter)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::Airstrike),
            Some(HostSuperweaponKind::A10Strike)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::ScudStorm),
            Some(HostSuperweaponKind::ScudStorm)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::ParticleCannon),
            Some(HostSuperweaponKind::ParticleCannon)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::NuclearMissile),
            Some(HostSuperweaponKind::NuclearMissile)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::AnthraxBomb),
            Some(HostSuperweaponKind::AnthraxBomb)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::SpectreGunship),
            Some(HostSuperweaponKind::SpectreGunship)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::CarpetBomb),
            Some(HostSuperweaponKind::CarpetBomb)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::Artillery),
            Some(HostSuperweaponKind::ArtilleryBarrage)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::CruiseMissile),
            Some(HostSuperweaponKind::CruiseMissile)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::RadarScan),
            None
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::SpySatellite),
            None
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::CiaIntelligence),
            None
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::EmpPulse),
            None
        );
    }

    #[test]
    fn nuclear_missile_params_match_retail_blast6() {
        let kind = HostSuperweaponKind::NuclearMissile;
        assert_eq!(kind.impact_delay_frames(), 180);
        assert!((kind.max_damage() - 3500.0).abs() < 0.1);
        assert!((kind.damage_radius() - 210.0).abs() < 0.1);
        assert!((kind.falloff_inner() - 60.0).abs() < 0.1);
        assert!(kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!HostSuperweaponKind::DaisyCutter.spawns_radiation());
    }

    #[test]
    fn anthrax_bomb_params_match_retail_weapon() {
        let kind = HostSuperweaponKind::AnthraxBomb;
        assert_eq!(kind.impact_delay_frames(), 90);
        assert!((kind.max_damage() - 200.0).abs() < 0.1);
        assert!((kind.damage_radius() - 100.0).abs() < 0.1);
        assert!((kind.falloff_inner() - 100.0).abs() < 0.1);
        assert!(kind.spawns_toxin_field());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_orbit_field());
        assert!(!HostSuperweaponKind::DaisyCutter.spawns_toxin_field());
        assert_eq!(ANTHRAX_TOXIN_DAMAGE_PER_TICK, 40.0);
        assert_eq!(ANTHRAX_TOXIN_RADIUS, 300.0);
        assert_eq!(ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES, 15);
        assert_eq!(ANTHRAX_TOXIN_DURATION_FRAMES, 1800);
    }

    #[test]
    fn spectre_gunship_params_match_retail_orbit() {
        let kind = HostSuperweaponKind::SpectreGunship;
        assert_eq!(kind.impact_delay_frames(), 90);
        assert!((kind.max_damage() - 0.0).abs() < 0.1);
        assert!((kind.damage_radius() - SPECTRE_ORBIT_RADIUS).abs() < 0.1);
        assert!(kind.spawns_orbit_field());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!kind.spawns_beam_field());
        assert!(!HostSuperweaponKind::DaisyCutter.spawns_orbit_field());
        assert_eq!(SPECTRE_ORBIT_DAMAGE_PER_TICK, 80.0);
        assert_eq!(SPECTRE_ORBIT_RADIUS, 200.0);
        assert_eq!(SPECTRE_ORBIT_TICK_INTERVAL_FRAMES, 9);
        assert_eq!(SPECTRE_ORBIT_DURATION_FRAMES, 450);
    }

    #[test]
    fn particle_cannon_params_match_retail_continuous_beam() {
        let kind = HostSuperweaponKind::ParticleCannon;
        assert_eq!(kind.impact_delay_frames(), 120);
        // Continuous beam residual: no one-shot impact blast.
        assert!((kind.max_damage() - 0.0).abs() < 0.1);
        assert!((kind.damage_radius() - PARTICLE_BEAM_RADIUS).abs() < 0.1);
        assert!(kind.spawns_beam_field());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!kind.spawns_orbit_field());
        assert!(!HostSuperweaponKind::DaisyCutter.spawns_beam_field());
        // damagePerPulse = (105/30 * 400) / 40 = 35
        assert!((PARTICLE_BEAM_DAMAGE_PER_PULSE - 35.0).abs() < 0.01);
        assert_eq!(PARTICLE_BEAM_RADIUS, 50.0);
        assert_eq!(PARTICLE_BEAM_TICK_INTERVAL_FRAMES, 3);
        assert_eq!(PARTICLE_BEAM_DURATION_FRAMES, 105);
        assert_eq!(PARTICLE_BEAM_TOTAL_PULSES, 40);
        // SwathOfDeath + DamageRadiusScalar retail residual.
        assert!((PARTICLE_SWATH_OF_DEATH_DISTANCE - 200.0).abs() < 0.1);
        assert!((PARTICLE_SWATH_OF_DEATH_AMPLITUDE - 50.0).abs() < 0.1);
        assert!((PARTICLE_DAMAGE_RADIUS_SCALAR - 3.4).abs() < 0.01);
        // WidthGrow grow/hold/decay + RevealRange + ScorchMarks retail residual.
        assert_eq!(PARTICLE_WIDTH_GROW_FRAMES, 60);
        assert_eq!(
            PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES,
            PARTICLE_BEAM_DURATION_FRAMES + PARTICLE_WIDTH_GROW_FRAMES
        );
        assert!((PARTICLE_REVEAL_RANGE - 50.0).abs() < 0.01);
        assert_eq!(PARTICLE_TOTAL_SCORCH_MARKS, 20);
        assert!((PARTICLE_SCORCH_MARK_SCALAR - 2.4).abs() < 0.01);
        assert!((PARTICLE_MANUAL_DRIVING_SPEED - 20.0).abs() < 0.01);
        assert!((PARTICLE_MANUAL_FAST_DRIVING_SPEED - 40.0).abs() < 0.01);
        assert_eq!(PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES, 15);
        // Intensity schedule retail residual.
        assert_eq!(PARTICLE_BEGIN_CHARGE_FRAMES, 150);
        assert_eq!(PARTICLE_RAISE_ANTENNA_FRAMES, 140);
        assert_eq!(PARTICLE_READY_DELAY_FRAMES, 60);
        assert_eq!(PARTICLE_BEAM_TRAVEL_FRAMES, 75);
        assert_eq!(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES, 30);
        assert!(PARTICLE_BEAM_LAUNCH_FX.contains("BeamLaunch"));
        // OuterBeamWidth × scalar / retail laser radius formula residual.
        assert!((PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - 26.0).abs() < 0.01);
        assert!((PARTICLE_ORBITAL_LASER_INNER_BEAM_WIDTH - 0.6).abs() < 0.01);
        assert_eq!(PARTICLE_ORBITAL_LASER_NUM_BEAMS, 12);
        assert!((PARTICLE_ORBITAL_LASER_SCROLL_RATE + 1.75).abs() < 0.01);
        assert!((PARTICLE_ORBITAL_LASER_TILING_SCALAR - 0.15).abs() < 0.01);
        assert_eq!(PARTICLE_ORBITAL_LASER_TEXTURE, "EXNoise02.tga");
        assert!((PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH - 1.2).abs() < 0.01);
        assert!((PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH - 2.0).abs() < 0.01);
        assert!((particle_orbital_laser_template_width() - 13.0).abs() < 0.01);
        assert!((particle_orbital_laser_current_radius(100, 160) - 13.0).abs() < 0.01);
        assert!((particle_orbital_laser_draw_width(100, 160) - 26.0).abs() < 0.01);
        assert!((particle_retail_damage_radius(100, 160) - 44.2).abs() < 0.05);
        assert!((particle_orbital_laser_draw_width(100, 130) - 13.0).abs() < 0.01);
        assert!((particle_retail_damage_radius(100, 130) - 22.1).abs() < 0.05);
        // Client-effects residual matrix honesty.
        let charging = particle_client_effects_for_status(ParticleUplinkStatus::Charging);
        assert_eq!(charging.outer_intensity, ParticleIntensity::Light);
        assert_eq!(charging.connector_lasers, 0);
        let preparing = particle_client_effects_for_status(ParticleUplinkStatus::Preparing);
        assert_eq!(preparing.outer_intensity, ParticleIntensity::Medium);
        let almost = particle_client_effects_for_status(ParticleUplinkStatus::AlmostReady);
        assert_eq!(almost.connector_intensity, ParticleIntensity::Medium);
        assert_eq!(almost.connector_lasers, PARTICLE_OUTER_EFFECT_NUM_BONES);
        let ready = particle_client_effects_for_status(ParticleUplinkStatus::ReadyToFire);
        assert_eq!(ready.laser_base_intensity, ParticleIntensity::Light);
        let firing = particle_client_effects_for_status(ParticleUplinkStatus::Firing);
        assert_eq!(firing.outer_intensity, ParticleIntensity::Intense);
        assert_eq!(firing.ground_to_orbit, 1);
        let postfire = particle_client_effects_for_status(ParticleUplinkStatus::Postfire);
        assert_eq!(postfire.outer_intensity, ParticleIntensity::Medium);
        assert_eq!(postfire.ground_to_orbit, 1);
        // Grow phase.
        assert!((particle_width_scalar(100, 100) - 0.0).abs() < 0.01);
        assert!((particle_width_scalar(100, 130) - 0.5).abs() < 0.01);
        assert!((particle_width_scalar(100, 160) - 1.0).abs() < 0.01);
        assert!((particle_beam_damage_radius(100, 160) - PARTICLE_BEAM_RADIUS).abs() < 0.01);
        // Hold through TotalFiringTime (decay start inclusive).
        let decay_start = particle_decay_start_frame(100);
        assert_eq!(decay_start, 100 + PARTICLE_BEAM_DURATION_FRAMES);
        assert!((particle_width_scalar(100, decay_start) - 1.0).abs() < 0.01);
        // Decay half-way: scalar 0.5, death at orbital lifetime.
        let half_decay = decay_start + PARTICLE_WIDTH_GROW_FRAMES / 2;
        assert!((particle_width_scalar(100, half_decay) - 0.5).abs() < 0.01);
        assert!(
            (particle_beam_damage_radius(100, half_decay) - 25.0).abs() < 0.1
        );
        let death = particle_death_frame(100);
        assert_eq!(death, 100 + PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES);
        assert!((particle_width_scalar(100, death) - 0.0).abs() < 0.01);
        assert_eq!(particle_next_scorch_frame(100, 0), 101);
        assert_eq!(
            particle_next_scorch_frame(100, 10),
            100 + (0.5 * PARTICLE_BEAM_DURATION_FRAMES as f32).floor() as u32
        );
        // First pulse (factor 0): cx = -distance/2.
        let o0 = particle_swath_offset(0);
        assert!((o0.x + PARTICLE_SWATH_OF_DEATH_DISTANCE * 0.5).abs() < 0.1);
        assert!(o0.z.abs() < 0.01);
        // Mid pulse (factor 0.5): at click epicenter offset.
        let mid_idx = PARTICLE_BEAM_TOTAL_PULSES / 2;
        let o_mid = particle_swath_offset(mid_idx);
        assert!(o_mid.x.abs() < 1.0, "mid swath along-axis near 0, got {}", o_mid.x);
        // Fractional nextFactor schedule residual.
        assert_eq!(particle_next_pulse_frame(100, 0), 101); // strict forward when 0
        assert_eq!(
            particle_next_pulse_frame(100, 20),
            100 + (0.5 * PARTICLE_BEAM_DURATION_FRAMES as f32).floor() as u32
        );
    }

    #[test]
    fn particle_cannon_impact_spawns_beam_and_ticks_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(100.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::China,
            target,
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::ParticleCannon));
        assert_eq!(reg.get(id).unwrap().impact_frame, 120);
        assert!(reg.beam_fields().is_empty());

        // First pulse swath epicenter = target + (-100, 0, 0) = (0, 0, 0).
        let swath0 = particle_swath_epicenter(target, 0);
        assert!((swath0.x - 0.0).abs() < 0.1);
        let objects = vec![
            (ObjectId(1), Vec3::new(-500.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), swath0, Team::GLA, true), // first-pulse swath epicenter
            (ObjectId(3), Vec3::new(30.0, 0.0, 0.0), Team::GLA, true), // in radius of swath0
            (ObjectId(4), Vec3::new(500.0, 0.0, 0.0), Team::GLA, true), // far
            (ObjectId(5), swath0, Team::China, true), // friendly
        ];

        // Charge residual: no impact plan before frame 120.
        assert!(reg.plan_due_impacts(119, &objects).is_empty());
        let impact_plans = reg.plan_due_impacts(120, &objects);
        assert_eq!(impact_plans.len(), 1);
        // Continuous beam: no one-shot impact hits.
        assert!(impact_plans[0].hits.is_empty());

        reg.record_impact_complete(id, 0.0, 0, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::ParticleCannon));
        assert!(reg.honesty_beam_ok());
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::ParticleCannon));
        assert_eq!(reg.beam_fields().len(), 1);
        assert_eq!(reg.beam_fields()[0].parent_strike_id, id);

        // First beam pulse on spawn frame — uses SwathOfDeath epicenter.
        // WidthGrow residual: radius 0 at spawn → only exact-epicenter unit hits.
        let beam_plans = reg.plan_due_beam_ticks(120, &objects);
        assert_eq!(beam_plans.len(), 1);
        assert!(
            (beam_plans[0].position.x - swath0.x).abs() < 0.1,
            "first pulse must use swath epicenter"
        );
        assert!((beam_plans[0].damage_radius - 0.0).abs() < 0.01);
        assert!((beam_plans[0].width_scalar - 0.0).abs() < 0.01);
        assert_eq!(beam_plans[0].hits.len(), 1); // epicenter only under width=0
        assert_eq!(beam_plans[0].hits[0].target_id, ObjectId(2));
        assert!(!beam_plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        assert!(!beam_plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(!beam_plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)));

        reg.record_beam_tick_complete(
            beam_plans[0].field_id,
            PARTICLE_BEAM_DAMAGE_PER_PULSE * 1.0,
            1,
            0,
            120,
        );
        assert!(reg.honesty_beam_damage_ok());
        assert!(reg.honesty_beam_swath_ok());
        assert!(reg.beam_fields()[0].swath_applications >= 1);
        assert!(reg.beam_fields()[0].max_swath_offset > 50.0);
        // WidthGrow residual: first pulse at spawn still records peak scalar 0.
        assert!(reg.beam_fields()[0].peak_width_scalar < 0.01);
        // Fractional nextFactor: pulses_made=1 → factor 1/40 * 105 = 2.625 → floor 2.
        let expected_next = particle_next_pulse_frame(120, 1).max(121);
        assert_eq!(reg.beam_fields()[0].next_tick_frame, expected_next);
        assert_eq!(reg.beam_fields()[0].pulses_made, 1);

        // Not due again until scheduled frame.
        assert!(reg.plan_due_beam_ticks(expected_next.saturating_sub(1), &objects).is_empty());
        let later = reg.plan_due_beam_ticks(expected_next, &objects);
        assert_eq!(later.len(), 1);
    }

    #[test]
    fn particle_uplink_swath_of_death_residual_honesty() {
        // Swath walks from -distance/2 to +distance/2 with sine lateral amplitude.
        let o_start = particle_swath_offset(0);
        let o_end = particle_swath_offset(PARTICLE_BEAM_TOTAL_PULSES);
        assert!((o_start.x + 100.0).abs() < 0.1);
        assert!((o_end.x - 100.0).abs() < 0.1);
        // Lateral amplitude peaks near quarter / three-quarter factor.
        let o_q = particle_swath_offset(PARTICLE_BEAM_TOTAL_PULSES / 4);
        assert!(
            o_q.z.abs() > 40.0,
            "quarter-swath lateral amplitude expected near 50, got {}",
            o_q.z
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::China,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;

        // Enemy parked at click epicenter: first pulse swath is at x=-100 → miss;
        // mid pulse swath returns near origin → hit.
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), Vec3::ZERO, Team::GLA, true),
        ];
        let first = reg.plan_due_beam_ticks(spawn, &objects);
        assert_eq!(first.len(), 1);
        assert!(
            first[0].hits.is_empty(),
            "click-epicenter unit must miss first swath pulse at x=-100"
        );
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, spawn);

        // Advance pulses to mid (factor ≈ 0.5).
        let mut frame = reg.beam_fields()[0].next_tick_frame;
        while reg.beam_fields()[0].pulses_made < PARTICLE_BEAM_TOTAL_PULSES / 2 {
            let plans = reg.plan_due_beam_ticks(frame, &objects);
            if plans.is_empty() {
                frame = frame.saturating_add(1);
                continue;
            }
            let hits = plans[0].hits.len() as u32;
            let dmg = PARTICLE_BEAM_DAMAGE_PER_PULSE * hits as f32;
            reg.record_beam_tick_complete(field_id, dmg, hits, 0, frame);
            frame = reg.beam_fields()[0].next_tick_frame;
        }
        // Mid swath should have hit click-epicenter unit at least once.
        assert!(
            reg.beam_fields()[0].damage_applications > 0,
            "mid swath residual must damage unit at click epicenter"
        );
        assert!(reg.honesty_beam_swath_ok());
        assert!(reg.beam_fields()[0].max_swath_offset > 50.0);
    }

    #[test]
    fn carpet_bomb_params_match_retail_multi_strike() {
        let kind = HostSuperweaponKind::CarpetBomb;
        assert_eq!(kind.impact_delay_frames(), CARPET_BOMB_IMPACT_DELAY_FRAMES);
        assert!((kind.max_damage() - CARPET_BOMB_DAMAGE).abs() < 0.1);
        assert!((kind.damage_radius() - CARPET_BOMB_RADIUS).abs() < 0.1);
        assert!((kind.falloff_inner() - CARPET_BOMB_RADIUS).abs() < 0.1);
        assert!(kind.is_line_multi_strike());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!kind.spawns_orbit_field());
        assert!(!kind.spawns_beam_field());
        assert!(!HostSuperweaponKind::DaisyCutter.is_line_multi_strike());
        assert_eq!(CARPET_BOMB_COUNT, 15);
        assert!((CARPET_BOMB_SPACING - 25.0).abs() < 0.1);
        assert!((CARPET_BOMB_DROP_VARIANCE_X - 30.0).abs() < 0.01);
        assert!((CARPET_BOMB_DROP_VARIANCE_Y - 40.0).abs() < 0.01);
        assert!((CARPET_BOMB_DROP_VARIANCE_Z - 0.0).abs() < 0.01);
        assert_eq!(CARPET_BOMB_DROP_DELAY_FRAMES, 9);
        // DropDelay residual: bomb i at approach + i * DropDelay.
        assert_eq!(carpet_bomb_impact_frame(0, 0), CARPET_BOMB_IMPACT_DELAY_FRAMES);
        assert_eq!(
            carpet_bomb_impact_frame(0, 1),
            CARPET_BOMB_IMPACT_DELAY_FRAMES + CARPET_BOMB_DROP_DELAY_FRAMES
        );
        assert_eq!(
            multi_strike_last_impact_frame(
                HostSuperweaponKind::CarpetBomb,
                0,
                ArtilleryBarrageScienceTier::Level1
            ),
            carpet_bomb_impact_frame(0, CARPET_BOMB_COUNT - 1)
        );
        let points = carpet_bomb_points(Vec3::new(100.0, 0.0, 50.0));
        assert_eq!(points.len(), CARPET_BOMB_COUNT as usize);
        // Base line still centered; DropVariance residual scatters within ±var.
        let base_center_x = 100.0;
        assert!(
            (points[7].x - base_center_x).abs() <= CARPET_BOMB_DROP_VARIANCE_X + 0.1,
            "center bomb DropVariance residual within X variance"
        );
        assert!(
            (points[0].x - (100.0 - 7.0 * CARPET_BOMB_SPACING)).abs()
                <= CARPET_BOMB_DROP_VARIANCE_X + 0.1
        );
        assert!(
            (points[14].x - (100.0 + 7.0 * CARPET_BOMB_SPACING)).abs()
                <= CARPET_BOMB_DROP_VARIANCE_X + 0.1
        );
        // Non-zero lateral scatter residual (Z from C++ Y variance).
        let any_z_scatter = points.iter().any(|p| (p.z - 50.0).abs() > 0.01);
        assert!(any_z_scatter, "DropVariance residual must scatter Z");
        for p in &points {
            assert!((p.z - 50.0).abs() <= CARPET_BOMB_DROP_VARIANCE_Y + 0.1);
        }
    }

    #[test]
    fn carpet_bomb_delayed_line_multi_strike_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::CarpetBomb,
            ObjectId(1),
            Team::China,
            target,
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::CarpetBomb));
        assert_eq!(
            reg.get(id).unwrap().impact_frame,
            CARPET_BOMB_IMPACT_DELAY_FRAMES
        );

        // Place enemies at DropVariance-adjusted residual epicenters.
        let points = carpet_bomb_points(target);
        let first = points[0];
        let center = points[7];
        let outer = points[14];
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), center, Team::USA, true), // center bomb (with variance)
            (ObjectId(3), outer, Team::USA, true),  // outer bomb (with variance)
            (ObjectId(4), Vec3::new(0.0, 0.0, 500.0), Team::USA, true), // far off-line
            (ObjectId(5), center, Team::China, true), // friendly
            (ObjectId(6), first, Team::USA, true),  // first bomb DropDelay residual
        ];

        // Before first bomb: no damage plan.
        assert!(reg
            .plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        // First DropDelay wave: only bomb 0 due — not complete.
        let first_wave = reg.plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(first_wave.len(), 1);
        assert_eq!(first_wave[0].wave_shell_count, 1);
        assert!(!first_wave[0].is_final_wave);
        assert!(first_wave[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(6) && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));
        // Center (index 7) and outer (index 14) not yet due.
        assert!(!first_wave[0].hits.iter().any(|h| h.target_id == ObjectId(2)));
        assert!(!first_wave[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        reg.record_impact_wave(
            id,
            CARPET_BOMB_DAMAGE,
            1,
            0,
            first_wave[0].wave_shell_count,
            first_wave[0].is_final_wave,
            &first_wave[0].epicenters,
        );
        assert!(!reg.honesty_complete_ok(HostSuperweaponKind::CarpetBomb));

        // Jump to last bomb frame: remaining bombs (incl. center + outer) apply.
        let last = multi_strike_last_impact_frame(
            HostSuperweaponKind::CarpetBomb,
            0,
            ArtilleryBarrageScienceTier::Level1,
        );
        let plans = reg.plan_due_impacts(last, &objects);
        assert_eq!(plans.len(), 1);
        assert!(plans[0].is_final_wave);
        assert!(plans[0].wave_shell_count >= 14);
        // Center + outer-bomb enemies + friendly (ALLIES residual); far excluded.
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
            && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)
            && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));

        reg.record_impact_wave(
            id,
            CARPET_BOMB_DAMAGE * 2.0,
            2,
            0,
            plans[0].wave_shell_count,
            plans[0].is_final_wave,
            &plans[0].epicenters,
        );
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::CarpetBomb));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::CarpetBomb));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
        assert!(reg.beam_fields().is_empty());
        assert_eq!(
            reg.get(id).unwrap().multi_strike_applied,
            CARPET_BOMB_COUNT
        );
    }

    #[test]
    fn carpet_bomb_drop_variance_residual_bounds() {
        // C++ Random(-var, +var) residual bounds for host deterministic scatter.
        for i in 0..CARPET_BOMB_COUNT {
            let o = drop_variance_offset(
                i,
                CARPET_BOMB_DROP_VARIANCE_X,
                CARPET_BOMB_DROP_VARIANCE_Y,
                CARPET_BOMB_DROP_VARIANCE_Z,
            );
            assert!(o.x.abs() <= CARPET_BOMB_DROP_VARIANCE_X + 0.001);
            assert!(o.z.abs() <= CARPET_BOMB_DROP_VARIANCE_Y + 0.001);
            assert!((o.y - 0.0).abs() < 0.001, "Z variance 0 → host Y 0");
        }
        // Supply OCL has no DropVariance — zero residual is identity.
        let zero = drop_variance_offset(3, 0.0, 0.0, 0.0);
        assert!((zero.x).abs() < 0.001 && (zero.y).abs() < 0.001 && (zero.z).abs() < 0.001);
    }

    #[test]
    fn artillery_barrage_params_match_retail_multi_shell() {
        let kind = HostSuperweaponKind::ArtilleryBarrage;
        assert_eq!(
            kind.impact_delay_frames(),
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
        );
        assert!((kind.max_damage() - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1);
        assert!((kind.damage_radius() - ARTILLERY_BARRAGE_RADIUS).abs() < 0.1);
        assert!((kind.falloff_inner() - ARTILLERY_BARRAGE_RADIUS).abs() < 0.1);
        assert!(kind.is_scatter_multi_strike());
        assert!(kind.is_multi_strike());
        assert!(!kind.is_line_multi_strike());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!kind.spawns_orbit_field());
        assert!(!HostSuperweaponKind::DaisyCutter.is_scatter_multi_strike());
        assert_eq!(ARTILLERY_BARRAGE_SHELL_COUNT, 12);
        assert_eq!(ARTILLERY_BARRAGE_SHELL_COUNT_L2, 24);
        assert_eq!(ARTILLERY_BARRAGE_SHELL_COUNT_L3, 36);
        assert_eq!(ArtilleryBarrageScienceTier::Level1.formation_size(), 12);
        assert_eq!(ArtilleryBarrageScienceTier::Level2.formation_size(), 24);
        assert_eq!(ArtilleryBarrageScienceTier::Level3.formation_size(), 36);
        assert_eq!(
            ArtilleryBarrageScienceTier::from_science_name("SCIENCE_ArtilleryBarrage3"),
            Some(ArtilleryBarrageScienceTier::Level3)
        );
        assert_eq!(
            ArtilleryBarrageScienceTier::highest_from_sciences([
                "SCIENCE_ArtilleryBarrage1",
                "SCIENCE_ArtilleryBarrage2",
            ]),
            ArtilleryBarrageScienceTier::Level2
        );
        assert!((ARTILLERY_BARRAGE_ERROR_RADIUS - 100.0).abs() < 0.1);
        assert!((ARTILLERY_BARRAGE_RING_RADIUS - 75.0).abs() < 0.1);
        // Lead shell DelayDelivery residual is 0; others in [0, max].
        assert_eq!(delay_delivery_frames(0, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES), 0);
        for i in 1..12 {
            let d = delay_delivery_frames(i, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
            assert!(d <= ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
        }
        // WeaponErrorRadius residual: index 0 spot-on; others within error radius.
        assert_eq!(weapon_error_radius_offset(0, ARTILLERY_BARRAGE_ERROR_RADIUS), Vec3::ZERO);
        let points = artillery_barrage_points(Vec3::new(100.0, 0.0, 50.0));
        assert_eq!(points.len(), ARTILLERY_BARRAGE_SHELL_COUNT as usize);
        // First shell at target; remaining scattered inside WeaponErrorRadius.
        assert!((points[0].x - 100.0).abs() < 0.1);
        assert!((points[0].z - 50.0).abs() < 0.1);
        let mut any_scatter = false;
        for p in points.iter().skip(1) {
            let dist = horizontal_distance(*p, Vec3::new(100.0, 0.0, 50.0));
            assert!(
                dist <= ARTILLERY_BARRAGE_ERROR_RADIUS + 0.1,
                "WeaponErrorRadius shell dist={dist}"
            );
            if dist > 0.5 {
                any_scatter = true;
            }
        }
        assert!(any_scatter, "WeaponErrorRadius residual must scatter non-lead shells");
        let points_l3 = artillery_barrage_points_for_tier(
            Vec3::new(0.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level3,
        );
        assert_eq!(points_l3.len(), 36);
    }

    #[test]
    fn artillery_barrage_delayed_multi_shell_scatter_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ArtilleryBarrage,
            ObjectId(1),
            Team::China,
            target,
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::ArtilleryBarrage));
        assert_eq!(
            reg.get(id).unwrap().impact_frame,
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
        );

        // Shells: center + WeaponErrorRadius residual scatter for index 1.
        let points = artillery_barrage_points(target);
        let outer = points[1];
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // center shell
            (ObjectId(3), outer, Team::USA, true),                    // scatter shell
            (ObjectId(4), Vec3::new(0.0, 0.0, 500.0), Team::USA, true), // far
            (ObjectId(5), Vec3::new(0.0, 0.0, 0.0), Team::China, true), // friendly
        ];

        // Before impact: no damage plan.
        assert!(reg
            .plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        // First wave: lead shell (DelayDelivery 0) — center hit; not necessarily final.
        let first = reg.plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(first.len(), 1);
        assert!(first[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2)
                && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1));
        reg.record_impact_wave(
            id,
            ARTILLERY_BARRAGE_DAMAGE,
            1,
            0,
            first[0].wave_shell_count,
            first[0].is_final_wave,
            &first[0].epicenters,
        );

        // Jump to last DelayDelivery shell frame: remaining scatter shells apply.
        let last = multi_strike_last_impact_frame(
            HostSuperweaponKind::ArtilleryBarrage,
            0,
            ArtilleryBarrageScienceTier::Level1,
        );
        let plans = reg.plan_due_impacts(last, &objects);
        if first[0].is_final_wave {
            // All shells had DelayDelivery 0 — already complete.
            assert!(reg.honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage));
        } else {
            assert_eq!(plans.len(), 1);
            assert!(plans[0].is_final_wave);
            // Scatter-shell enemy hit when its shell is due; far excluded; ALLIES residual allows friendly.
            assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
                && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1)
                || first[0]
                    .hits
                    .iter()
                    .any(|h| h.target_id == ObjectId(3)));
            assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
            // Friendly at center may take shell damage under RadiusDamageAffects ALLIES.
            let _friendly_ok = plans[0].hits.iter().any(|h| h.target_id == ObjectId(5))
                || first[0].hits.iter().any(|h| h.target_id == ObjectId(5));
            reg.record_impact_wave(
                id,
                ARTILLERY_BARRAGE_DAMAGE,
                1,
                0,
                plans[0].wave_shell_count,
                plans[0].is_final_wave,
                &plans[0].epicenters,
            );
            assert!(reg.honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage));
        }
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::ArtilleryBarrage));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
        assert_eq!(
            reg.get(id).unwrap().multi_strike_applied,
            ARTILLERY_BARRAGE_SHELL_COUNT
        );
    }

    #[test]
    fn weapon_error_radius_and_delay_delivery_residual_honesty() {
        // C++: formationIndex 0 spot-on; others Random(0, r) + Random(0, 2π).
        assert_eq!(
            weapon_error_radius_offset(0, ARTILLERY_BARRAGE_ERROR_RADIUS),
            Vec3::ZERO
        );
        for i in 1..36 {
            let o = weapon_error_radius_offset(i, ARTILLERY_BARRAGE_ERROR_RADIUS);
            let dist = (o.x * o.x + o.z * o.z).sqrt();
            assert!(dist <= ARTILLERY_BARRAGE_ERROR_RADIUS + 0.001);
            assert!((o.y).abs() < 0.001);
        }
        // DelayDelivery: lead 0; others in [0, max].
        assert_eq!(delay_delivery_frames(0, 90), 0);
        let mut any_positive = false;
        for i in 1..36 {
            let d = delay_delivery_frames(i, 90);
            assert!(d <= 90);
            if d > 0 {
                any_positive = true;
            }
        }
        assert!(any_positive, "DelayDelivery residual must stagger some shells");
        // Shell impact frames: base + delay.
        assert_eq!(
            artillery_shell_impact_frame(10, 0),
            10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
        );
        assert!(
            artillery_shell_impact_frame(10, 5)
                >= 10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
        );
    }

    #[test]
    fn cruise_missile_params_match_retail_moab() {
        let kind = HostSuperweaponKind::CruiseMissile;
        assert_eq!(
            kind.impact_delay_frames(),
            CRUISE_MISSILE_IMPACT_DELAY_FRAMES
        );
        assert!((kind.max_damage() - CRUISE_MISSILE_DAMAGE).abs() < 0.1);
        assert!((kind.damage_radius() - CRUISE_MISSILE_RADIUS).abs() < 0.1);
        assert!((kind.falloff_inner() - CRUISE_MISSILE_FALLOFF_INNER).abs() < 0.1);
        assert!(!kind.is_multi_strike());
        assert!(!kind.spawns_radiation());
        assert!(!kind.spawns_toxin_field());
        assert!(!kind.spawns_orbit_field());
        assert!(kind.spawns_moab_flame());
        assert!(kind.hits_allies());
        assert!(HostSuperweaponKind::DaisyCutter.spawns_moab_flame());
        assert!((MOAB_FLAME_DAMAGE - 5.0).abs() < 0.01);
        assert!((MOAB_FLAME_RADIUS - 100.0).abs() < 0.1);
        assert_eq!(kind.activate_audio(), "SuperweaponCruiseMissile");
        assert_eq!(kind.impact_audio(), "CruiseMissileImpact");
        assert_eq!(CRUISE_MISSILE_IMPACT_DELAY_FRAMES, 180);
        assert!((CRUISE_MISSILE_DAMAGE - 2000.0).abs() < 0.1);
        assert!((CRUISE_MISSILE_RADIUS - 150.0).abs() < 0.1);
    }

    #[test]
    fn cruise_missile_delayed_area_damage_after_loft() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::CruiseMissile,
            ObjectId(1),
            Team::USA,
            target,
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::CruiseMissile));
        assert_eq!(
            reg.get(id).unwrap().impact_frame,
            CRUISE_MISSILE_IMPACT_DELAY_FRAMES
        );

        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::new(0.0, 0.0, 0.0), Team::GLA, true), // epicenter
            (ObjectId(3), Vec3::new(50.0, 0.0, 0.0), Team::GLA, true), // inside radius
            (ObjectId(4), Vec3::new(500.0, 0.0, 0.0), Team::GLA, true), // far
            (ObjectId(5), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // friendly (ALLIES residual)
        ];

        // Before impact: no damage plan.
        assert!(reg
            .plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        let plans = reg.plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        // Epicenter + near enemy + friendly (ALLIES residual); far excluded.
        // Epicenter damage = MOAB primary + MOABFlame secondary residual.
        let expected_epicenter = CRUISE_MISSILE_DAMAGE + MOAB_FLAME_DAMAGE;
        assert_eq!(plans[0].hits.len(), 3);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - expected_epicenter).abs() < 0.1));
        assert!(plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3) && h.damage > MOAB_FLAME_DAMAGE));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)
            && (h.damage - expected_epicenter).abs() < 0.1));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));

        reg.record_impact_complete(id, expected_epicenter * 2.0, 3, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::CruiseMissile));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::CruiseMissile));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
    }

    #[test]
    fn moab_flame_and_allies_residual_honesty() {
        // MOABFlameWeapon residual on DaisyCutter / CruiseMissile only.
        assert!(HostSuperweaponKind::DaisyCutter.spawns_moab_flame());
        assert!(HostSuperweaponKind::CruiseMissile.spawns_moab_flame());
        assert!(!HostSuperweaponKind::CarpetBomb.spawns_moab_flame());
        assert!(!HostSuperweaponKind::ArtilleryBarrage.spawns_moab_flame());
        // RadiusDamageAffects ALLIES residual for retail blast kinds.
        assert!(HostSuperweaponKind::ArtilleryBarrage.hits_allies());
        assert!(HostSuperweaponKind::CarpetBomb.hits_allies());
        assert!(HostSuperweaponKind::NuclearMissile.hits_allies());
        assert!(HostSuperweaponKind::AnthraxBomb.hits_allies());
        // Continuous field kinds keep their own filters (not primary blast ALLIES).
        assert!(!HostSuperweaponKind::SpectreGunship.hits_allies());
        assert!(!HostSuperweaponKind::ParticleCannon.hits_allies());

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::DaisyCutter,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::ZERO, Team::GLA, true),
            (ObjectId(3), Vec3::new(80.0, 0.0, 0.0), Team::USA, true), // ally in flame radius
            (ObjectId(4), Vec3::new(160.0, 0.0, 0.0), Team::USA, true), // ally outside flame, in outer blast
        ];
        let plans = reg.plan_due_impacts(90, &objects);
        assert_eq!(plans.len(), 1);
        // Ally + enemy hit (ALLIES residual); source excluded.
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        // Epicenter enemy: primary + flame.
        let epic = plans[0]
            .hits
            .iter()
            .find(|h| h.target_id == ObjectId(2))
            .unwrap();
        assert!((epic.damage - (2000.0 + MOAB_FLAME_DAMAGE)).abs() < 0.1);
        // Outer ally at 160: falloff primary only (outside flame 100).
        let outer = plans[0]
            .hits
            .iter()
            .find(|h| h.target_id == ObjectId(4))
            .unwrap();
        assert!(outer.damage > 0.0 && outer.damage < 2000.0);
        assert!((outer.damage - MOAB_FLAME_DAMAGE).abs() > 1.0 || outer.damage < MOAB_FLAME_DAMAGE);
        // Flame residual alone would be 5; falloff primary at 160 should be non-trivial.
        let primary_only =
            HostSpecialPowerStrikeRegistry::damage_at_distance(HostSuperweaponKind::DaisyCutter, 160.0);
        assert!((outer.damage - primary_only).abs() < 0.1);
        let _ = id;
    }

    #[test]
    fn spectre_gunship_impact_spawns_orbit_and_ticks_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::SpectreGunship));
        assert_eq!(reg.get(id).unwrap().impact_frame, 90);

        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 100.0), Team::GLA, true),
            (ObjectId(3), Vec3::new(100.0, 0.0, 100.0), Team::USA, true), // friendly
            (ObjectId(4), Vec3::new(900.0, 0.0, 900.0), Team::GLA, true),
        ];

        // Before orbit insertion: no plan, no orbit field.
        assert!(reg.plan_due_impacts(89, &objects).is_empty());
        assert!(reg.orbit_fields().is_empty());

        let plans = reg.plan_due_impacts(90, &objects);
        assert_eq!(plans.len(), 1);
        // No one-shot blast residual (max_damage = 0).
        assert!(plans[0].hits.is_empty());

        reg.record_impact_complete(id, 0.0, 0, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::SpectreGunship));
        assert!(reg.honesty_orbit_ok());
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::SpectreGunship));
        assert_eq!(reg.orbit_fields().len(), 1);
        assert_eq!(reg.orbit_fields()[0].parent_strike_id, id);
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.radiation_fields().is_empty());

        // First orbit tick: howitzer (r25 at reticle) + gattling (nearest enemy).
        // Enemy at field position: both residual streams hit.
        let orbit_plans = reg.plan_due_orbit_ticks(90, &objects);
        assert_eq!(orbit_plans.len(), 1);
        assert_eq!(orbit_plans[0].hits.len(), 1);
        assert_eq!(orbit_plans[0].hits[0].target_id, ObjectId(2));
        let expected_first = SPECTRE_ORBIT_DAMAGE_PER_TICK + SPECTRE_GATTLING_DAMAGE;
        assert!(
            (orbit_plans[0].hits[0].damage - expected_first).abs() < 0.01,
            "first tick howitzer+gattling residual, got {}",
            orbit_plans[0].hits[0].damage
        );

        reg.record_orbit_tick_complete(orbit_plans[0].field_id, expected_first, 1, 0, 90);
        assert!(reg.honesty_orbit_damage_ok());
        assert!(reg.honesty_gattling_ok());
        assert_eq!(reg.orbit_fields()[0].howitzer_ticks, 1);
        assert_eq!(reg.orbit_fields()[0].gattling_ticks, 1);
        assert_eq!(
            reg.orbit_fields()[0].next_tick_frame,
            90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES
        );
        assert_eq!(
            reg.orbit_fields()[0].next_gattling_tick_frame,
            90 + SPECTRE_GATTLING_TICK_INTERVAL_FRAMES
        );

        // Gattling-only tick after 3 frames (howitzer still waiting).
        let gattling_only = reg.plan_due_orbit_ticks(90 + SPECTRE_GATTLING_TICK_INTERVAL_FRAMES, &objects);
        assert_eq!(gattling_only.len(), 1);
        assert_eq!(gattling_only[0].hits.len(), 1);
        assert!(
            (gattling_only[0].hits[0].damage - SPECTRE_GATTLING_DAMAGE).abs() < 0.01
        );
        reg.record_orbit_tick_complete(
            gattling_only[0].field_id,
            SPECTRE_GATTLING_DAMAGE,
            1,
            0,
            90 + SPECTRE_GATTLING_TICK_INTERVAL_FRAMES,
        );

        // Howitzer residual after HowitzerFiringRate interval.
        let later = reg.plan_due_orbit_ticks(90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES, &objects);
        assert_eq!(later.len(), 1);
        assert!(!later[0].hits.is_empty());
    }

    #[test]
    fn anthrax_bomb_impact_spawns_toxin_and_ticks_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::AnthraxBomb,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::AnthraxBomb));
        assert_eq!(reg.get(id).unwrap().impact_frame, 90);

        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::GLA, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 100.0), Team::USA, true),
            (ObjectId(3), Vec3::new(100.0, 0.0, 100.0), Team::GLA, true), // friendly at epicenter
            (ObjectId(4), Vec3::new(900.0, 0.0, 900.0), Team::USA, true),
        ];

        // Before impact: no plan, no toxin.
        assert!(reg.plan_due_impacts(89, &objects).is_empty());
        assert!(reg.toxin_fields().is_empty());

        let plans = reg.plan_due_impacts(90, &objects);
        assert_eq!(plans.len(), 1);
        // Blast residual hits ALLIES ENEMIES NEUTRALS (retail RadiusDamageAffects).
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - 200.0).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
            && (h.damage - 200.0).abs() < 0.1));

        reg.record_impact_complete(id, 400.0, 2, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::AnthraxBomb));
        assert!(reg.honesty_toxin_ok());
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::AnthraxBomb));
        assert_eq!(reg.toxin_fields().len(), 1);
        assert_eq!(reg.toxin_fields()[0].parent_strike_id, id);
        assert!(reg.radiation_fields().is_empty());

        // Toxin tick hits all teams in radius (retail ALLIES ENEMIES NEUTRALS).
        let tox_plans = reg.plan_due_toxin_ticks(90, &objects);
        assert_eq!(tox_plans.len(), 1);
        // source (1) excluded; epicenter USA (2) + GLA friendly (3) hit; far (4) not.
        assert_eq!(tox_plans[0].hits.len(), 2);
        assert!(tox_plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - ANTHRAX_TOXIN_DAMAGE_PER_TICK).abs() < 0.01));
        assert!(tox_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3)));

        reg.record_toxin_tick_complete(tox_plans[0].field_id, 80.0, 2, 0, 90);
        assert!(reg.honesty_toxin_damage_ok());
        assert_eq!(reg.toxin_fields()[0].next_tick_frame, 90 + 15);
    }

    #[test]
    fn nuclear_missile_impact_spawns_radiation_and_ticks_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::NuclearMissile,
            ObjectId(1),
            Team::China,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::NuclearMissile));
        assert_eq!(reg.get(id).unwrap().impact_frame, 180);

        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 100.0), Team::USA, true),
            (ObjectId(3), Vec3::new(100.0, 0.0, 100.0), Team::China, true), // friendly at epicenter
            (ObjectId(4), Vec3::new(900.0, 0.0, 900.0), Team::USA, true),
        ];

        // Before impact: no plan, no radiation.
        assert!(reg.plan_due_impacts(179, &objects).is_empty());
        assert!(reg.radiation_fields().is_empty());

        let plans = reg.plan_due_impacts(180, &objects);
        assert_eq!(plans.len(), 1);
        // Blast residual hits ALLIES ENEMIES NEUTRALS (retail RadiusDamageAffects).
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - 3500.0).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
            && (h.damage - 3500.0).abs() < 0.1));

        reg.record_impact_complete(id, 7000.0, 2, 1);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::NuclearMissile));
        assert!(reg.honesty_radiation_ok());
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::NuclearMissile));
        assert_eq!(reg.radiation_fields().len(), 1);
        assert_eq!(reg.radiation_fields()[0].parent_strike_id, id);

        // Radiation tick hits all teams in radius (retail ALLIES ENEMIES NEUTRALS).
        let rad_plans = reg.plan_due_radiation_ticks(180, &objects);
        assert_eq!(rad_plans.len(), 1);
        // source (1) excluded; epicenter USA (2) + China friendly (3) hit; far (4) not.
        assert_eq!(rad_plans[0].hits.len(), 2);
        assert!(rad_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2)
                && (h.damage - NUKE_RADIATION_DAMAGE_PER_TICK).abs() < 0.01));
        assert!(rad_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3)));

        reg.record_radiation_tick_complete(rad_plans[0].field_id, 50.0, 2, 0, 180);
        assert!(reg.honesty_radiation_damage_ok());
        assert_eq!(reg.radiation_fields()[0].next_tick_frame, 180 + 23);
    }

    #[test]
    fn queue_and_complete_daisy_cutter_damage_plan() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::DaisyCutter,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::DaisyCutter));
        assert!(!reg.honesty_complete_ok(HostSuperweaponKind::DaisyCutter));

        let strike = reg.get(id).expect("strike");
        assert_eq!(strike.impact_frame, 90);
        assert_eq!(strike.phase, HostStrikePhase::Queued);

        // Before impact frame: no plans.
        let objects = vec![
            (ObjectId(1), Vec3::new(0.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 100.0), Team::GLA, true),
            (ObjectId(3), Vec3::new(500.0, 0.0, 500.0), Team::GLA, true),
        ];
        assert!(reg.plan_due_impacts(89, &objects).is_empty());

        let plans = reg.plan_due_impacts(90, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        // Primary Daisy/MOAB blast + MOABFlameWeapon secondary residual.
        assert!((plans[0].hits[0].damage - (2000.0 + MOAB_FLAME_DAMAGE)).abs() < 0.01);

        reg.record_impact_complete(id, 2000.0 + MOAB_FLAME_DAMAGE, 1, 1);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::DaisyCutter));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::DaisyCutter));
        assert_eq!(reg.get(id).unwrap().phase, HostStrikePhase::Completed);
    }

    #[test]
    fn falloff_two_stage_matches_fab_shape() {
        let kind = HostSuperweaponKind::DaisyCutter;
        assert!((HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 0.0) - 2000.0).abs() < 0.1);
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 100.0) - 2000.0).abs() < 0.1
        );
        let mid = HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 135.0);
        assert!((mid - 1000.0).abs() < 1.0, "mid falloff expected ~1000, got {mid}");
        assert_eq!(
            HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 170.0),
            0.0
        );
    }

    #[test]
    fn friendly_fire_allies_residual_and_source_excluded() {
        // A10 retail RadiusDamageAffects includes ALLIES — friendly is hit.
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        reg.queue(
            HostSuperweaponKind::A10Strike,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::new(5.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(3), Vec3::new(5.0, 0.0, 0.0), Team::China, true),
        ];
        let plans = reg.plan_due_impacts(60, &objects);
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        // Source launcher still excluded.
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(1)));
    }

    #[test]
    fn restore_from_snapshot_keeps_pending_impact_frame() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::DaisyCutter,
            ObjectId(9),
            Team::USA,
            Vec3::new(1.0, 0.0, 2.0),
            10,
        );
        let snap = reg.strikes_snapshot();
        let next = reg.next_id();

        let mut loaded = HostSpecialPowerStrikeRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.pending_count(), 1);
        let s = loaded.get(id).expect("restored strike");
        assert_eq!(s.impact_frame, 100);
        assert_eq!(s.phase, HostStrikePhase::Queued);
        assert_eq!(loaded.next_id(), next);
    }
    #[test]
    fn scud_storm_multi_missile_scatter_and_poison_residual() {
        // ClipSize 9 + ScatterTarget + primary/secondary + LargePoisonField.
        assert_eq!(SCUD_STORM_MISSILE_COUNT, 9);
        assert!((SCUD_STORM_SCATTER_SCALAR - 120.0).abs() < 0.1);
        assert!((SCUD_STORM_PRIMARY_DAMAGE - 500.0).abs() < 0.1);
        assert!((SCUD_STORM_PRIMARY_RADIUS - 50.0).abs() < 0.1);
        assert!((SCUD_STORM_SECONDARY_DAMAGE - 150.0).abs() < 0.1);
        assert!((SCUD_STORM_SECONDARY_RADIUS - 200.0).abs() < 0.1);
        assert_eq!(SCUD_STORM_PRE_ATTACK_FRAMES, 90);
        assert!((SCUD_STORM_POISON_DAMAGE_PER_TICK - 15.0).abs() < 0.1);
        assert!((SCUD_STORM_POISON_RADIUS - 140.0).abs() < 0.1);
        assert_eq!(SCUD_STORM_POISON_DURATION_FRAMES, 1350);

        let kind = HostSuperweaponKind::ScudStorm;
        assert!(kind.is_scud_multi_strike());
        assert!(kind.is_multi_strike());
        assert!(kind.spawns_toxin_field());
        assert!(kind.spawns_scud_poison_field());
        assert!(!HostSuperweaponKind::AnthraxBomb.spawns_scud_poison_field());
        assert_eq!(kind.impact_delay_frames(), SCUD_STORM_PRE_ATTACK_FRAMES);
        assert!((kind.max_damage() - SCUD_STORM_PRIMARY_DAMAGE).abs() < 0.1);

        // Primary/secondary step residual.
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 0.0)
                - SCUD_STORM_PRIMARY_DAMAGE)
                .abs()
                < 0.1
        );
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 50.0)
                - SCUD_STORM_PRIMARY_DAMAGE)
                .abs()
                < 0.1
        );
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 51.0)
                - SCUD_STORM_SECONDARY_DAMAGE)
                .abs()
                < 0.1
        );
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 200.0)
                - SCUD_STORM_SECONDARY_DAMAGE)
                .abs()
                < 0.1
        );
        assert!(
            HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 201.0).abs() < 0.1
        );

        let target = Vec3::new(100.0, 0.0, 50.0);
        let points = scud_storm_points(target);
        assert_eq!(points.len(), SCUD_STORM_MISSILE_COUNT as usize);
        // First scatter entry (0, 0.133) * 120 → offset z ≈ 15.96
        assert!((points[0].x - 100.0).abs() < 0.1);
        assert!((points[0].z - (50.0 + 0.133 * 120.0)).abs() < 0.1);
        // Fifth entry (0.767, 0) * 120
        assert!((points[4].x - (100.0 + 0.767 * 120.0)).abs() < 0.1);
        assert!((points[4].z - 50.0).abs() < 0.1);

        // Stagger residual: first at PreAttack; later missiles later.
        assert_eq!(scud_missile_impact_frame(0, 0), SCUD_STORM_PRE_ATTACK_FRAMES);
        assert!(scud_missile_impact_frame(0, 1) > scud_missile_impact_frame(0, 0));
        assert!(scud_missile_impact_frame(0, 8) > scud_missile_impact_frame(0, 1));
        let last = multi_strike_last_impact_frame(
            kind,
            0,
            ArtilleryBarrageScienceTier::Level1,
        );
        assert_eq!(last, scud_missile_impact_frame(0, SCUD_STORM_MISSILE_COUNT - 1));

        // Multi-wave impact + LargePoisonField on complete.
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(kind, ObjectId(1), Team::GLA, target, 0);
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::GLA, true),
            // Near first scatter epicenter (primary).
            (
                ObjectId(2),
                Vec3::new(points[0].x, 0.0, points[0].z),
                Team::USA,
                true,
            ),
            // Ally at same epicenter (ALLIES residual).
            (
                ObjectId(3),
                Vec3::new(points[0].x, 0.0, points[0].z),
                Team::GLA,
                true,
            ),
        ];

        // Before first missile: nothing.
        assert!(reg.plan_due_impacts(SCUD_STORM_PRE_ATTACK_FRAMES - 1, &objects).is_empty());

        // First missile wave.
        let plans = reg.plan_due_impacts(SCUD_STORM_PRE_ATTACK_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        assert!(!plans[0].is_final_wave);
        assert!(plans[0].wave_shell_count >= 1);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - SCUD_STORM_PRIMARY_DAMAGE).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        reg.record_impact_wave(
            id,
            SCUD_STORM_PRIMARY_DAMAGE * 2.0,
            2,
            0,
            plans[0].wave_shell_count,
            false,
            &plans[0].epicenters,
        );
        assert!(
            !reg.toxin_fields().is_empty(),
            "first Scud missile wave must spawn LargePoisonField residual"
        );
        let poison_after_first = reg.toxin_fields().len();

        // Jump to last missile: complete + more poison.
        let last_plans = reg.plan_due_impacts(last, &objects);
        assert_eq!(last_plans.len(), 1);
        assert!(last_plans[0].is_final_wave);
        reg.record_impact_wave(
            id,
            100.0,
            1,
            0,
            last_plans[0].wave_shell_count,
            true,
            &last_plans[0].epicenters,
        );
        assert!(reg.honesty_complete_ok(kind));
        assert!(reg.honesty_toxin_ok());
        assert!(
            reg.toxin_fields().len() > poison_after_first,
            "later Scud missiles must spawn additional LargePoisonField residual"
        );
        let field = &reg.toxin_fields()[0];
        assert!((field.damage_per_tick - SCUD_STORM_POISON_DAMAGE_PER_TICK).abs() < 0.1);
        assert!((field.radius - SCUD_STORM_POISON_RADIUS).abs() < 0.1);
        assert_eq!(field.tick_interval_frames, SCUD_STORM_POISON_TICK_INTERVAL_FRAMES);
        assert_eq!(
            field.expires_frame,
            field.spawn_frame + SCUD_STORM_POISON_DURATION_FRAMES
        );

        // Poison tick uses LargePoison residual damage (one plan per field).
        let tox = reg.plan_due_toxin_ticks(field.spawn_frame, &objects);
        assert!(!tox.is_empty());
        assert!(tox.iter().any(|plan| {
            plan.hits.iter().any(|h| {
                h.target_id == ObjectId(2)
                    && (h.damage - SCUD_STORM_POISON_DAMAGE_PER_TICK).abs() < 0.01
            })
        }));
        // ClipSize-9 per-missile residual can spawn up to 9 fields.
        assert!(reg.toxin_fields().len() <= SCUD_STORM_MISSILE_COUNT as usize);
        assert!(reg.toxin_fields_spawned_total() >= 2);
    }

    #[test]
    fn spectre_orbit_time_science_tier_residual() {
        assert_eq!(SpectreGunshipScienceTier::Level1.orbit_duration_frames(), 300);
        assert_eq!(SpectreGunshipScienceTier::Level2.orbit_duration_frames(), 450);
        assert_eq!(SpectreGunshipScienceTier::Level3.orbit_duration_frames(), 600);
        assert_eq!(
            SpectreGunshipScienceTier::from_science_name("SCIENCE_SpectreGunship3"),
            Some(SpectreGunshipScienceTier::Level3)
        );
        assert_eq!(
            SpectreGunshipScienceTier::from_science_name("SCIENCE_SpectreGunship1"),
            Some(SpectreGunshipScienceTier::Level1)
        );
        assert_eq!(
            SpectreGunshipScienceTier::highest_from_sciences([
                "SCIENCE_SpectreGunship1",
                "SCIENCE_SpectreGunship2",
            ]),
            SpectreGunshipScienceTier::Level2
        );
        assert_eq!(
            SpectreGunshipScienceTier::highest_from_sciences([
                "SCIENCE_SpectreGunship1",
                "SCIENCE_SpectreGunship3",
            ]),
            SpectreGunshipScienceTier::Level3
        );
        // No spectre science → default Level2 (retail 15s OrbitTime).
        assert_eq!(
            SpectreGunshipScienceTier::highest_from_sciences(["SCIENCE_Rank3"]),
            SpectreGunshipScienceTier::Level2
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue_with_tiers(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
            ArtilleryBarrageScienceTier::Level1,
            SpectreGunshipScienceTier::Level3,
        );
        assert_eq!(reg.get(id).unwrap().spectre_tier, SpectreGunshipScienceTier::Level3);
        reg.record_impact_complete(id, 0.0, 0, 0);
        assert_eq!(reg.orbit_fields().len(), 1);
        assert_eq!(
            reg.orbit_fields()[0].expires_frame,
            90 + SpectreGunshipScienceTier::Level3.orbit_duration_frames()
        );

        // Level1 shorter orbit residual.
        let mut reg2 = HostSpecialPowerStrikeRegistry::new();
        let id2 = reg2.queue_with_tiers(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
            ArtilleryBarrageScienceTier::Level1,
            SpectreGunshipScienceTier::Level1,
        );
        reg2.record_impact_complete(id2, 0.0, 0, 0);
        assert_eq!(
            reg2.orbit_fields()[0].expires_frame,
            90 + SpectreGunshipScienceTier::Level1.orbit_duration_frames()
        );
    }


    #[test]
    fn spectre_gattling_and_howitzer_residual_honesty() {
        assert_eq!(SPECTRE_HOWITZER_RADIUS, 25.0);
        assert_eq!(SPECTRE_HOWITZER_RANDOM_OFFSET, 20.0);
        assert_eq!(SPECTRE_GATTLING_DAMAGE, 90.0);
        assert_eq!(SPECTRE_GATTLING_TICK_INTERVAL_FRAMES, 3);
        // Offset residual stays within RandomOffsetForHowitzer.
        for i in 0..16 {
            let o = spectre_howitzer_offset(i);
            assert!(o.x.abs() <= SPECTRE_HOWITZER_RANDOM_OFFSET + 1e-3);
            assert!(o.z.abs() <= SPECTRE_HOWITZER_RANDOM_OFFSET + 1e-3);
            assert!(o.y.abs() < 1e-5);
        }

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);

        // Enemy far from reticle (outside howitzer 25) but inside orbit 200:
        // gattling only.
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 0.0), Team::GLA, true),
            (ObjectId(3), Vec3::new(10.0, 0.0, 0.0), Team::GLA, true), // near reticle
        ];
        let plans = reg.plan_due_orbit_ticks(90, &objects);
        assert_eq!(plans.len(), 1);
        // Near enemy: howitzer (possibly offset) and/or gattling nearest.
        // Far enemy at 100: only gattling if nearer than 3? nearest is 3 at dist 10.
        // Gattling picks nearest = ObjectId(3) at ~10.
        // Howitzer: epicenter near 0 with offset ≤20; ObjectId(3) at 10 may be in r25.
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        // Object 2 at 100 is outside howitzer and not nearest for gattling.
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)));
    }

    #[test]
    fn scud_storm_anthrax_upgrade_secondary_and_poison_residual() {
        // Base residual.
        assert!((ScudStormAnthraxTier::Base.secondary_damage() - 150.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::Base.poison_damage_per_tick() - 15.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::Base.primary_damage() - 500.0).abs() < 0.1);
        // Anthrax Beta upgraded: Secondary 200 + poison 25.
        assert!((ScudStormAnthraxTier::AnthraxBeta.secondary_damage() - 200.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::AnthraxBeta.poison_damage_per_tick() - 25.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::AnthraxBeta.primary_damage() - 500.0).abs() < 0.1);
        // Chem Gamma: Primary 550 + Secondary 200 + poison 25.
        assert!((ScudStormAnthraxTier::AnthraxGamma.primary_damage() - 550.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::AnthraxGamma.secondary_damage() - 200.0).abs() < 0.1);
        assert!((ScudStormAnthraxTier::AnthraxGamma.poison_damage_per_tick() - 25.0).abs() < 0.1);

        assert_eq!(
            ScudStormAnthraxTier::highest_from_upgrades(["Upgrade_GLAAnthraxBeta"]),
            ScudStormAnthraxTier::AnthraxBeta
        );
        assert_eq!(
            ScudStormAnthraxTier::highest_from_upgrades([
                "Upgrade_GLAAnthraxBeta",
                "Chem_Upgrade_GLAAnthraxGamma",
            ]),
            ScudStormAnthraxTier::AnthraxGamma
        );
        assert_eq!(
            ScudStormAnthraxTier::highest_from_upgrades(["SCIENCE_Rank3"]),
            ScudStormAnthraxTier::Base
        );

        // Damage step residual for upgraded Secondary 200.
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance_with_scud_tier(
                HostSuperweaponKind::ScudStorm,
                100.0,
                ScudStormAnthraxTier::AnthraxBeta,
            ) - 200.0)
                .abs()
                < 0.1
        );
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance_with_scud_tier(
                HostSuperweaponKind::ScudStorm,
                0.0,
                ScudStormAnthraxTier::AnthraxGamma,
            ) - 550.0)
                .abs()
                < 0.1
        );

        // Host path: queue with Beta → secondary 200 hit + poison 25 field.
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue_with_all_tiers(
            HostSuperweaponKind::ScudStorm,
            ObjectId(1),
            Team::GLA,
            target,
            0,
            ArtilleryBarrageScienceTier::Level1,
            SpectreGunshipScienceTier::Level2,
            ScudStormAnthraxTier::AnthraxBeta,
        );
        assert_eq!(
            reg.get(id).unwrap().scud_anthrax_tier,
            ScudStormAnthraxTier::AnthraxBeta
        );
        let points = scud_storm_points(target);
        // Unit in secondary ring (between 50 and 200) of first epicenter.
        let secondary_pos = Vec3::new(points[0].x + 80.0, 0.0, points[0].z);
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::GLA, true),
            (ObjectId(2), secondary_pos, Team::USA, true),
        ];
        let plans = reg.plan_due_impacts(SCUD_STORM_PRE_ATTACK_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        assert!(plans[0].hits.iter().any(|h| {
            h.target_id == ObjectId(2) && (h.damage - SCUD_STORM_SECONDARY_DAMAGE_UPGRADED).abs() < 0.1
        }));
        reg.record_impact_wave(
            id,
            SCUD_STORM_SECONDARY_DAMAGE_UPGRADED,
            1,
            0,
            plans[0].wave_shell_count,
            false,
            &plans[0].epicenters,
        );
        assert!(!reg.toxin_fields().is_empty());
        let field = &reg.toxin_fields()[0];
        assert!((field.damage_per_tick - SCUD_STORM_POISON_DAMAGE_UPGRADED).abs() < 0.1);
        assert!((field.radius - SCUD_STORM_POISON_RADIUS).abs() < 0.1);
    }

    #[test]
    fn spectre_continuous_fire_rof_residual_honesty() {
        // Interval residual: base 3; MEAN floor(3/2)=1; FAST floor(3/3)=1.
        assert_eq!(spectre_gattling_interval_frames(0), 3);
        assert_eq!(spectre_gattling_interval_frames(1), 3);
        assert_eq!(spectre_gattling_interval_frames(2), 1); // > ContinuousFireOne=1
        assert_eq!(spectre_gattling_interval_frames(3), 1); // > ContinuousFireTwo=2
        // Howitzer: base 9; MEAN floor(9/1.5)=6; FAST floor(9/2)=4.
        assert_eq!(spectre_howitzer_interval_frames(0), 9);
        assert_eq!(spectre_howitzer_interval_frames(1), 9);
        assert_eq!(spectre_howitzer_interval_frames(2), 6);
        assert_eq!(spectre_howitzer_interval_frames(3), 4);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        assert_eq!(reg.orbit_fields().len(), 1);
        let field_id = reg.orbit_fields()[0].id;
        let spawn = reg.orbit_fields()[0].spawn_frame;

        // Tick 1: base interval scheduled after.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_consecutive, 1);
            assert_eq!(f.howitzer_consecutive, 1);
            assert_eq!(f.gattling_fire_level, 0);
            assert_eq!(f.next_gattling_tick_frame, spawn + 3);
            assert_eq!(f.next_tick_frame, spawn + 9);
        }

        // Tick 2 at spawn+3: consecutive → MEAN for gattling.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 3);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_consecutive, 2);
            assert_eq!(f.gattling_fire_level, 1);
            assert_eq!(f.next_gattling_tick_frame, spawn + 3 + 1);
            // Howitzer not due at +3 (next is spawn+9).
            assert_eq!(f.howitzer_consecutive, 1);
        }
        assert!(reg.honesty_gattling_continuous_fire_ok());

        // Third gattling tick → FAST + VoiceRapidFire residual cue.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 4);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_consecutive, 3);
            assert_eq!(f.gattling_fire_level, 2);
            assert!(f.rapid_fire_voice_cues >= 1);
        }
        assert!(reg.honesty_voice_rapid_fire_ok());
        assert_eq!(SPECTRE_VOICE_RAPID_FIRE_AUDIO, "SpectreGunshipVoiceRapidFire");
        assert!(reg.honesty_model_condition_continuous_fire_ok());
        assert!(reg.orbit_fields()[0].model_condition_mean_sets >= 1);
        assert!(reg.orbit_fields()[0].model_condition_fast_sets >= 1);

        // Advance howitzer to MEAN at spawn+9.
        reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn + 9);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.howitzer_consecutive, 2);
            assert_eq!(f.howitzer_fire_level, 1);
            assert_eq!(f.next_tick_frame, spawn + 9 + 6);
        }
        assert!(reg.honesty_howitzer_continuous_fire_ok());
    }

    #[test]
    fn spectre_continuous_fire_coast_cooldown_residual() {
        // ContinuousFireCoast = 2000 ms → 60 frames @ 30 FPS.
        assert_eq!(SPECTRE_CONTINUOUS_FIRE_COAST_FRAMES, 60);
        // coast_until = frame + interval + coast
        assert_eq!(spectre_coast_until_after_shot(100, 3), 100 + 3 + 60);
        // Within coast window → no spin-down.
        assert!(spectre_coast_spin_down(50, 100, 2, 5).is_none());
        // Past coast with MEAN/FAST → cool to base.
        assert_eq!(spectre_coast_spin_down(101, 100, 2, 5), Some((0, 0)));
        // Past coast but already base + zero consecutive → no-op.
        assert!(spectre_coast_spin_down(101, 100, 0, 0).is_none());

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.orbit_fields()[0].id;
        let spawn = reg.orbit_fields()[0].spawn_frame;

        // Ramp gattling to FAST (3 consecutive shots).
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn);
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 3);
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 4);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_fire_level, 2);
            assert_eq!(f.gattling_consecutive, 3);
            assert!(f.gattling_coast_until_frame > spawn + 4);
        }
        let coast_until = reg.orbit_fields()[0].gattling_coast_until_frame;

        // Jump past ContinuousFireCoast without further shots → spin-down.
        reg.apply_orbit_coast_cooldown(coast_until + 1);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_consecutive, 0);
            assert_eq!(f.gattling_fire_level, 0);
            assert_eq!(f.gattling_coast_until_frame, 0);
            assert!(f.gattling_coast_applications >= 1);
            // Howitzer may also cool if its coast was armed earlier.
        }
        assert!(reg.honesty_continuous_fire_coast_ok());

        // After cool-down, next shot restarts at base interval residual.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, coast_until + 1);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_consecutive, 1);
            assert_eq!(f.gattling_fire_level, 0);
            assert_eq!(
                f.next_gattling_tick_frame,
                coast_until + 1 + SPECTRE_GATTLING_TICK_INTERVAL_FRAMES
            );
        }
        // MODELCONDITION_CONTINUOUS_FIRE_SLOW residual on coolDown.
        assert!(reg.honesty_model_condition_slow_ok());
        assert!(
            reg.orbit_fields()[0].model_condition_slow_sets >= 1,
            "coolDown must set CONTINUOUS_FIRE_SLOW residual"
        );
    }

    #[test]
    fn particle_uplink_damage_pulse_remnant_residual_honesty() {
        // Retail remnant weapon / lifetime residual constants.
        assert!((PARTICLE_REMNANT_DAMAGE_PER_TICK - 15.0).abs() < 0.01);
        assert!((PARTICLE_REMNANT_RADIUS - 10.0).abs() < 0.01);
        assert_eq!(PARTICLE_REMNANT_TICK_INTERVAL_FRAMES, 7);
        assert_eq!(PARTICLE_REMNANT_DURATION_FRAMES, 120);
        assert_eq!(PARTICLE_REMNANT_OBJECT_NAME, "ParticleUplinkCannonTrailRemnant");
        assert_eq!(
            PARTICLE_REMNANT_WEAPON_NAME,
            "ParticleUplinkCannonBeamTrailRemnantWeapon"
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        assert_eq!(reg.beam_fields().len(), 1);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;
        assert!(reg.remnant_fields().is_empty());

        // Completing a beam pulse spawns one remnant at the pulse swath epicenter.
        let first_epicenter = particle_swath_epicenter(click, 0);
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, spawn);
        assert_eq!(reg.remnant_fields().len(), 1);
        assert_eq!(reg.remnant_fields_spawned_total(), 1);
        assert!(reg.honesty_beam_remnant_ok());
        {
            let r = &reg.remnant_fields()[0];
            assert_eq!(r.parent_beam_id, field_id);
            assert_eq!(r.spawn_frame, spawn);
            assert_eq!(r.expires_frame, spawn + PARTICLE_REMNANT_DURATION_FRAMES);
            assert_eq!(r.next_tick_frame, spawn);
            let dx = (r.position.x - first_epicenter.x).abs();
            let dz = (r.position.z - first_epicenter.z).abs();
            assert!(dx < 0.01 && dz < 0.01, "remnant at first swath epicenter");
        }

        // Remnant damages living units in radius 10 (including same-team residual).
        // First swath epicenter is at x=-100 relative to click.
        let rem_pos = reg.remnant_fields()[0].position;
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), rem_pos, Team::USA, true), // ally in remnant radius
            (ObjectId(3), rem_pos + Vec3::new(50.0, 0.0, 0.0), Team::GLA, true),
        ];
        let plans = reg.plan_due_remnant_ticks(spawn, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - PARTICLE_REMNANT_DAMAGE_PER_TICK).abs() < 0.01);
        reg.record_remnant_tick_complete(plans[0].field_id, 15.0, 1, 0, spawn);
        assert!(reg.honesty_beam_remnant_damage_ok());
        assert_eq!(
            reg.remnant_fields()[0].next_tick_frame,
            spawn + PARTICLE_REMNANT_TICK_INTERVAL_FRAMES
        );

        // Second beam pulse → second remnant (trail residual accumulates).
        let next = reg.beam_fields()[0].next_tick_frame;
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, next);
        assert_eq!(reg.remnant_fields_spawned_total(), 2);
        assert_eq!(reg.remnant_fields().len(), 2);

        // Expire remnant after lifetime residual.
        reg.prune_expired_remnant(spawn + PARTICLE_REMNANT_DURATION_FRAMES);
        // First remnant expired; second may still be live if spawned later.
        assert!(
            reg.remnant_fields()
                .iter()
                .all(|f| f.spawn_frame > spawn || f.is_expired(spawn + PARTICLE_REMNANT_DURATION_FRAMES)
                    || f.expires_frame > spawn + PARTICLE_REMNANT_DURATION_FRAMES)
                || reg.remnant_fields().len() <= 1
        );
    }

    #[test]
    fn particle_uplink_width_grow_damage_radius_residual_honesty() {
        // WidthGrowTime 2000ms → 60 frames; radius ramps 0→PARTICLE_BEAM_RADIUS.
        assert_eq!(PARTICLE_WIDTH_GROW_FRAMES, 60);
        assert!((PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - 26.0).abs() < 0.01);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::China,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;

        // First-pulse swath epicenter at x=-100. Park unit 30 units from it.
        let epic0 = particle_swath_epicenter(click, 0);
        let near = epic0 + Vec3::new(30.0, 0.0, 0.0);
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), near, Team::GLA, true),
        ];

        // Spawn frame: width scalar 0 → miss (radius 0).
        let early = reg.plan_due_beam_ticks(spawn, &objects);
        assert_eq!(early.len(), 1);
        assert!(early[0].hits.is_empty());
        assert!((early[0].damage_radius - 0.0).abs() < 0.01);
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, spawn);

        // Advance to half grow (scalar 0.5 → radius 25) — still miss unit at 30.
        let half = spawn + PARTICLE_WIDTH_GROW_FRAMES / 2;
        // Force next tick due at half-grow frame.
        if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
            f.next_tick_frame = half;
            // Keep pulses_made so swath stays at first epicenter for radius test.
            f.pulses_made = 0;
        }
        let mid = reg.plan_due_beam_ticks(half, &objects);
        assert_eq!(mid.len(), 1);
        assert!((mid[0].width_scalar - 0.5).abs() < 0.01);
        assert!((mid[0].damage_radius - 25.0).abs() < 0.1);
        assert!(
            mid[0].hits.is_empty(),
            "half-grow radius 25 must miss unit at dist 30"
        );
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, half);
        assert!((reg.beam_fields()[0].peak_width_scalar - 0.5).abs() < 0.01);

        // Full grow: radius 50 → hit unit at dist 30.
        let full = spawn + PARTICLE_WIDTH_GROW_FRAMES;
        if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
            f.next_tick_frame = full;
            f.pulses_made = 0; // keep first swath epicenter
        }
        let late = reg.plan_due_beam_ticks(full, &objects);
        assert_eq!(late.len(), 1);
        assert!((late[0].width_scalar - 1.0).abs() < 0.01);
        assert!((late[0].damage_radius - PARTICLE_BEAM_RADIUS).abs() < 0.1);
        assert_eq!(late[0].hits.len(), 1);
        assert_eq!(late[0].hits[0].target_id, ObjectId(2));
        reg.record_beam_tick_complete(
            field_id,
            PARTICLE_BEAM_DAMAGE_PER_PULSE,
            1,
            0,
            full,
        );
        assert!(reg.honesty_beam_width_grow_ok());
        assert!((reg.beam_fields()[0].peak_width_scalar - 1.0).abs() < 0.01);
        assert!((reg.beam_fields()[0].last_damage_radius - PARTICLE_BEAM_RADIUS).abs() < 0.1);
    }

    #[test]
    fn particle_uplink_width_grow_decay_shrink_residual_honesty() {
        // After TotalFiringTime, WidthGrow decay shrinks scalar 1→0 over 60 frames
        // (retail LaserUpdate::setDecayFrames / LASERSTATUS_DECAYING).
        assert_eq!(PARTICLE_WIDTH_GROW_FRAMES, 60);
        assert_eq!(
            PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES,
            PARTICLE_BEAM_DURATION_FRAMES + PARTICLE_WIDTH_GROW_FRAMES
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::China,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;
        assert_eq!(
            reg.beam_fields()[0].expires_frame,
            particle_death_frame(spawn),
            "beam lives through WidthGrow decay tail"
        );

        // First-pulse swath epicenter; park unit 30 from it for radius tests.
        let epic0 = particle_swath_epicenter(click, 0);
        let near = epic0 + Vec3::new(30.0, 0.0, 0.0);
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), near, Team::GLA, true),
        ];

        // Hold phase end (TotalFiringTime): full radius 50 → hit unit at dist 30.
        let decay_start = particle_decay_start_frame(spawn);
        if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
            f.next_tick_frame = decay_start;
            f.pulses_made = 0;
            f.peak_width_scalar = 1.0; // prior grow residual reached full
        }
        let hold = reg.plan_due_beam_ticks(decay_start, &objects);
        assert_eq!(hold.len(), 1);
        assert!((hold[0].width_scalar - 1.0).abs() < 0.01);
        assert!((hold[0].damage_radius - PARTICLE_BEAM_RADIUS).abs() < 0.1);
        assert_eq!(hold[0].hits.len(), 1);
        reg.record_beam_tick_complete(
            field_id,
            PARTICLE_BEAM_DAMAGE_PER_PULSE,
            1,
            0,
            decay_start,
        );

        // Half-decay: scalar 0.5 → radius 25 → miss unit at dist 30.
        let half_decay = decay_start + PARTICLE_WIDTH_GROW_FRAMES / 2;
        if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
            f.next_tick_frame = half_decay;
            f.pulses_made = 0; // keep first swath epicenter for radius test
        }
        let mid = reg.plan_due_beam_ticks(half_decay, &objects);
        assert_eq!(mid.len(), 1);
        assert!((mid[0].width_scalar - 0.5).abs() < 0.01);
        assert!((mid[0].damage_radius - 25.0).abs() < 0.1);
        assert!(
            mid[0].hits.is_empty(),
            "half-decay radius 25 must miss unit at dist 30"
        );
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, half_decay);
        assert!(reg.beam_fields()[0].decay_samples > 0);
        assert!(reg.beam_fields()[0].trough_width_scalar < 0.51);
        assert!(reg.honesty_beam_width_decay_ok());

        // Sample-only path (no damage pulse) still tracks trough residual.
        let later = half_decay + 10;
        reg.sample_beam_width_honesty(later);
        assert!(reg.beam_fields()[0].trough_width_scalar < 0.4);
        assert!((reg.beam_fields()[0].last_width_scalar
            - particle_width_scalar(spawn, later))
            .abs()
            < 0.01);

        // Beam still alive during decay tail; dies at orbital death frame.
        assert!(!reg.beam_fields()[0].is_expired(later));
        let death = particle_death_frame(spawn);
        reg.prune_expired_beam(death);
        assert!(
            reg.beam_fields().is_empty(),
            "beam must expire after WidthGrow decay death"
        );
    }

    #[test]
    fn particle_uplink_scorch_reveal_residual_honesty() {
        // TotalScorchMarks 20 + RevealRange 50 + GroundHitFX residual.
        assert_eq!(PARTICLE_TOTAL_SCORCH_MARKS, 20);
        assert!((PARTICLE_REVEAL_RANGE - 50.0).abs() < 0.01);
        assert!(PARTICLE_GROUND_HIT_FX.contains("BeamHitsGround"));
        assert!((PARTICLE_SCORCH_MARK_SCALAR - 2.4).abs() < 0.01);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(10.0, 0.0, 5.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let spawn = reg.beam_fields()[0].spawn_frame;
        assert_eq!(reg.beam_fields()[0].scorch_marks_made, 0);
        assert_eq!(reg.beam_fields()[0].next_scorch_frame, spawn);

        // First scorch/reveal on spawn frame (retail m_nextScorchMarkFrame = now).
        let events = reg.apply_due_beam_scorch_reveals(spawn);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorch_mark_index, 1);
        assert!((events[0].reveal_range - PARTICLE_REVEAL_RANGE).abs() < 0.01);
        // First scorch uses pulse index 0 → first swath epicenter.
        let expected_pos = particle_swath_epicenter(click, 0);
        assert!((events[0].position.x - expected_pos.x).abs() < 0.1);
        assert!((events[0].position.z - expected_pos.z).abs() < 0.1);
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.scorch_marks_made, 1);
            assert_eq!(f.reveal_applications, 1);
            assert_eq!(f.ground_hit_fx_applications, 1);
            assert!(f.next_scorch_frame > spawn);
        }
        assert!(reg.honesty_beam_scorch_ok());
        assert!(reg.honesty_beam_reveal_ok());

        // Not due again until scheduled scorch frame.
        let next = reg.beam_fields()[0].next_scorch_frame;
        assert!(reg.apply_due_beam_scorch_reveals(next.saturating_sub(1)).is_empty());

        // Catch-up: jump past several scorch slots → multiple residual events.
        let late = spawn + PARTICLE_BEAM_DURATION_FRAMES;
        let caught = reg.apply_due_beam_scorch_reveals(late);
        assert!(caught.len() >= 5, "fractional scorch schedule catch-up, got {}", caught.len());
        assert!(
            reg.beam_fields()[0].scorch_marks_made <= PARTICLE_TOTAL_SCORCH_MARKS
        );
        assert_eq!(
            reg.beam_fields()[0].reveal_applications,
            reg.beam_fields()[0].scorch_marks_made
        );
        assert_eq!(
            reg.beam_fields()[0].ground_hit_fx_applications,
            reg.beam_fields()[0].scorch_marks_made
        );

        // Cap at TotalScorchMarks.
        let _ = reg.apply_due_beam_scorch_reveals(late + 1000);
        assert_eq!(
            reg.beam_fields()[0].scorch_marks_made,
            PARTICLE_TOTAL_SCORCH_MARKS
        );
    }

    #[test]
    fn spectre_model_condition_continuous_fire_residual_honesty() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.orbit_fields()[0].id;
        let spawn = reg.orbit_fields()[0].spawn_frame;

        // Base shot: no model-condition residual yet.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn);
        assert_eq!(reg.orbit_fields()[0].model_condition_mean_sets, 0);
        assert_eq!(reg.orbit_fields()[0].model_condition_fast_sets, 0);

        // MEAN residual set on ContinuousFireOne threshold.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 3);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_fire_level, 1);
            assert!(f.model_condition_mean_sets >= 1);
        }
        assert!(reg.honesty_model_condition_continuous_fire_ok());

        // FAST residual set on ContinuousFireTwo threshold.
        reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 4);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.gattling_fire_level, 2);
            assert!(f.model_condition_fast_sets >= 1);
            assert!(f.rapid_fire_voice_cues >= 1);
        }

        // Coast cool-down → CONTINUOUS_FIRE_SLOW residual.
        let coast_until = reg.orbit_fields()[0].gattling_coast_until_frame;
        reg.apply_orbit_coast_cooldown(coast_until + 1);
        assert!(reg.honesty_model_condition_slow_ok());
        assert!(reg.orbit_fields()[0].model_condition_slow_sets >= 1);
    }

    #[test]
    fn particle_uplink_intensity_schedule_and_beam_launch_fx_residual_honesty() {
        // Ready-countdown residual relative to ready_frame = 350.
        // beginCharge = 350 - 60 - 140 - 150 = 0
        // raiseAntenna = 150, almostReady = 290, ready = 350
        assert_eq!(
            particle_status_for_ready_countdown(0, 350),
            ParticleUplinkStatus::Charging
        );
        assert_eq!(
            particle_status_for_ready_countdown(150, 350),
            ParticleUplinkStatus::Preparing
        );
        assert_eq!(
            particle_status_for_ready_countdown(290, 350),
            ParticleUplinkStatus::AlmostReady
        );
        assert_eq!(
            particle_status_for_ready_countdown(350, 350),
            ParticleUplinkStatus::ReadyToFire
        );
        // Attack residual: FIRING → POSTFIRE → PACKING.
        assert_eq!(
            particle_status_for_attack(100, 100, 105, 60),
            ParticleUplinkStatus::Firing
        );
        assert_eq!(
            particle_status_for_attack(205, 100, 105, 60),
            ParticleUplinkStatus::Postfire
        );
        assert_eq!(
            particle_status_for_attack(265, 100, 105, 60),
            ParticleUplinkStatus::Packing
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        // Activate@0 / impact@120 → PREPARING residual seeded on queue.
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.particle_status, ParticleUplinkStatus::Preparing);
            assert!(s.particle_preparing_applications >= 1);
            assert!(s.particle_model_unpacking_sets >= 1);
        }

        // Advance through ALMOST_READY (impact-60 = 60).
        reg.advance_particle_intensity_schedule(60);
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.particle_status, ParticleUplinkStatus::AlmostReady);
            assert!(s.particle_almost_ready_applications >= 1);
            assert!(s.particle_model_deployed_sets >= 1);
        }

        // READY_TO_FIRE at impact frame, then complete → FIRING beam.
        reg.advance_particle_intensity_schedule(120);
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.particle_status, ParticleUplinkStatus::ReadyToFire);
            assert!(s.particle_ready_applications >= 1);
        }
        reg.record_impact_complete(id, 0.0, 0, 0);
        assert!(!reg.beam_fields().is_empty());
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.status, ParticleUplinkStatus::Firing);
            assert_eq!(f.outer_intensity, ParticleIntensity::Intense);
            assert_eq!(f.beam_launch_fx_applications, 1);
            assert_eq!(
                f.next_launch_fx_frame,
                f.spawn_frame + PARTICLE_LAUNCH_FX_INTERVAL_FRAMES
            );
        }
        assert!(reg.honesty_beam_intensity_schedule_ok());
        assert!(reg.honesty_beam_outer_nodes_ok());

        // BeamLaunchFX residual refresh after DelayBetweenLaunchFX.
        let spawn = reg.beam_fields()[0].spawn_frame;
        reg.advance_particle_intensity_schedule(spawn + PARTICLE_LAUNCH_FX_INTERVAL_FRAMES);
        assert!(reg.beam_fields()[0].beam_launch_fx_applications >= 2);
        assert!(reg.honesty_beam_launch_fx_ok());

        // POSTFIRE residual at TotalFiringTime.
        let decay = spawn + PARTICLE_BEAM_DURATION_FRAMES;
        reg.advance_particle_intensity_schedule(decay);
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.status, ParticleUplinkStatus::Postfire);
            assert_eq!(f.outer_intensity, ParticleIntensity::Medium);
            assert_eq!(f.connector_intensity, ParticleIntensity::Medium);
            assert!(f.postfire_applications >= 1);
            assert_eq!(f.ground_to_orbit_laser_created, 1);
        }
        assert!(reg.honesty_beam_postfire_ok());

        // PACKING residual at end of WidthGrow decay tail.
        let death = particle_death_frame(spawn);
        reg.advance_particle_intensity_schedule(death);
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.status, ParticleUplinkStatus::Packing);
            assert_eq!(f.outer_intensity, ParticleIntensity::None);
            assert!(f.packing_applications >= 1);
            assert_eq!(f.outer_node_systems_created, 0);
        }
    }

    #[test]
    fn scud_storm_pre_attack_and_chem_fx_residual_honesty() {
        assert_eq!(SCUD_STORM_CHEM_FX_BONE_COUNT, 3);
        assert_eq!(SCUD_STORM_LAUNCH_BONE, "WeaponA");
        assert!(SCUD_STORM_CHEM_FX_PARTICLE.contains("Goo"));
        assert!(SCUD_STORM_FIRE_FX.contains("ScudStormMissile"));
        assert!(SCUD_STORM_DETONATION_FX.contains("Detonation"));

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::ScudStorm,
            ObjectId(1),
            Team::GLA,
            Vec3::new(50.0, 0.0, 50.0),
            0,
        );
        {
            let s = reg.get(id).unwrap();
            assert!(s.scud_pre_attack_active);
            assert_eq!(s.scud_chem_fx_bones, SCUD_STORM_CHEM_FX_BONE_COUNT);
            assert!(s.scud_launch_bone_applications >= 1);
        }
        // PreAttack residual frames accumulate until first missile.
        for f in 1..SCUD_STORM_PRE_ATTACK_FRAMES {
            reg.advance_particle_intensity_schedule(f);
        }
        {
            let s = reg.get(id).unwrap();
            assert!(s.scud_pre_attack_active);
            assert!(s.scud_pre_attack_frames >= SCUD_STORM_PRE_ATTACK_FRAMES - 1);
        }
        assert!(reg.honesty_scud_pre_attack_and_chem_fx_ok());

        // First missile wave: PreAttack ends; FireFX + detonation residual.
        reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(50.0, 0.0, 50.0)]);
        {
            let s = reg.get(id).unwrap();
            assert!(!s.scud_pre_attack_active);
            assert!(s.scud_fire_fx_applications >= 1);
            assert!(s.scud_detonation_fx_applications >= 1);
        }
        assert!(reg.honesty_scud_pre_attack_and_chem_fx_ok());
    }

    #[test]
    fn particle_uplink_manual_drive_and_outer_nodes_residual_honesty() {
        // Manual drive speed residual: 20/s and 40/s → /30 frames.
        assert!((particle_manual_speed_per_frame(false) - (20.0 / 30.0)).abs() < 1e-4);
        assert!((particle_manual_speed_per_frame(true) - (40.0 / 30.0)).abs() < 1e-4);
        assert_eq!(PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES, 15);
        // Double-click gap: C++ last - 2ndLast < delay → fast.
        assert!(!particle_is_fast_drive(100, 0)); // first click after zero init
        assert!(particle_is_fast_drive(110, 100)); // 10 < 15
        assert!(!particle_is_fast_drive(120, 100)); // 20 >= 15
        // Outer-node residual retail honesty.
        assert_eq!(PARTICLE_OUTER_EFFECT_NUM_BONES, 5);
        assert_eq!(PARTICLE_OUTER_EFFECT_BONE_NAME, "FX");
        assert_eq!(PARTICLE_CONNECTOR_BONE_NAME, "FXConnector");
        assert_eq!(PARTICLE_FIRE_BONE_NAME, "FXMain");
        assert!(PARTICLE_OUTER_NODE_INTENSE_FLARE.contains("Intense"));
        assert!(PARTICLE_CONNECTOR_INTENSE_LASER.contains("Intense"));
        assert!(PARTICLE_ORBITAL_LASER_NAME.contains("OrbitalLaser"));

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;

        // STATUS_FIRING outer-node / connector residual on spawn.
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.outer_node_systems_created, PARTICLE_OUTER_EFFECT_NUM_BONES);
            assert_eq!(f.connector_lasers_created, PARTICLE_OUTER_EFFECT_NUM_BONES);
            assert_eq!(f.laser_base_flare_created, 1);
            assert_eq!(f.ground_to_orbit_laser_created, 1);
            assert!(!f.manual_target_mode);
        }
        assert!(reg.honesty_beam_outer_nodes_ok());

        // First pulse uses swath (not manual).
        let swath0 = particle_swath_epicenter(click, 0);
        let objects = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), swath0, Team::GLA, true),
        ];
        let first = reg.plan_due_beam_ticks(spawn, &objects);
        assert_eq!(first.len(), 1);
        assert!((first[0].position.x - swath0.x).abs() < 0.1);
        reg.record_beam_tick_complete(field_id, 0.0, 0, 0, spawn);
        assert!(reg.honesty_beam_swath_ok());

        // Arm manual override far from current swath epicenter.
        let override_dest = Vec3::new(200.0, 0.0, 0.0);
        assert!(reg.set_beam_override_destination(field_id, override_dest, spawn + 1));
        {
            let f = &reg.beam_fields()[0];
            assert!(f.manual_target_mode);
            assert_eq!(f.last_driving_click_frame, spawn + 1);
            // Seeded from last swath epicenter when entering manual.
            assert!((f.current_target_position.x - swath0.x).abs() < 0.1);
        }

        // Advance 30 frames at normal speed: 20 units/sec → 20 units moved.
        let after_normal = spawn + 1 + 30;
        reg.advance_manual_beam_drive(after_normal);
        {
            let f = &reg.beam_fields()[0];
            assert!(
                f.manual_drive_distance_total > 19.0 && f.manual_drive_distance_total < 21.0,
                "normal drive ~20 units over 1s, got {}",
                f.manual_drive_distance_total
            );
            assert!(f.manual_drive_applications >= 1);
            assert_eq!(f.fast_drive_applications, 0);
            // Still short of override (200 - (-100) = 300 remaining initially).
            assert!(f.current_target_position.x < override_dest.x - 1.0);
        }
        assert!(reg.honesty_beam_manual_drive_ok());

        // Double-click residual → fast drive (40 units/sec).
        // Second click ends the first retarget window; third click within 15
        // frames of the second arms ManualFastDrivingSpeed.
        let click2 = after_normal;
        assert!(reg.set_beam_override_destination(field_id, override_dest, click2));
        let click3 = click2 + 10; // gap 10 < 15
        assert!(reg.set_beam_override_destination(field_id, override_dest, click3));
        assert!(particle_is_fast_drive(click3, click2));
        // Sync drive update to click3 so the next advance measures exactly 30 frames.
        reg.advance_manual_beam_drive(click3);
        let before_fast_dist = reg.beam_fields()[0].manual_drive_distance_total;
        let before_fast_pos_x = reg.beam_fields()[0].current_target_position.x;
        let after_fast = click3 + 30;
        reg.advance_manual_beam_drive(after_fast);
        {
            let f = &reg.beam_fields()[0];
            let moved = f.manual_drive_distance_total - before_fast_dist;
            assert!(
                moved > 39.0 && moved < 41.0,
                "fast drive ~40 units over 1s, got {}",
                moved
            );
            assert!(f.fast_drive_applications >= 1);
            assert!(f.current_target_position.x > before_fast_pos_x);
        }
        assert!(reg.honesty_beam_fast_drive_ok());

        // Damage pulse under manual mode uses current_target_position, not swath.
        if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
            f.next_tick_frame = after_fast;
            f.pulses_made = 1; // keep non-zero; epicenter is manual now
        }
        let manual_pos = reg.beam_fields()[0].current_target_position;
        let objects_manual = vec![
            (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(3), manual_pos, Team::GLA, true),
            (ObjectId(4), swath0, Team::GLA, true), // old swath — should miss
        ];
        // Full width after grow (spawn + 60 already passed).
        let plans = reg.plan_due_beam_ticks(after_fast, &objects_manual);
        assert_eq!(plans.len(), 1);
        assert!((plans[0].position.x - manual_pos.x).abs() < 0.1);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
    }

    #[test]
    fn spectre_howitzer_shell_projectile_residual_honesty() {
        // Retail SpectreHowitzerShell / SpectreHowitzerGun projectile residual.
        assert_eq!(SPECTRE_HOWITZER_SHELL_OBJECT, "SpectreHowitzerShell");
        assert!((SPECTRE_HOWITZER_WEAPON_SPEED - 999.0).abs() < 0.01);
        assert_eq!(SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES, 30);
        assert!((SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT - 1.0).abs() < 0.01);
        assert!((SPECTRE_HOWITZER_SHELL_GEOMETRY_RADIUS - 4.0).abs() < 0.01);
        assert!((SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01);
        assert!((SPECTRE_HOWITZER_SHELL_LOCOMOTOR_SPEED - 1111.0).abs() < 0.01);
        assert!((SPECTRE_HOWITZER_SHELL_MASS - 1.0).abs() < 0.01);
        assert!((SPECTRE_HOWITZER_SHELL_GEOMETRY_HEIGHT - 4.0).abs() < 0.01);
        assert_eq!(SPECTRE_HOWITZER_SHELL_MODEL, "AVSpectreShell1");
        assert!(SPECTRE_HOWITZER_SHELL_HEIGHT_DIE_ONLY_MOVING_DOWN);
        assert!(SPECTRE_HOWITZER_SHELL_DEATH_DETONATED_FX.contains("NukeGLA"));
        assert!(SPECTRE_HOWITZER_SHELL_DEATH_LASERED_FX.contains("Disintegrate"));
        assert!(SPECTRE_HOWITZER_FIRE_FX.contains("TankGun"));
        assert!(SPECTRE_HOWITZER_DETONATION_FX.contains("SpectreHowitzer"));
        assert!(SPECTRE_HOWITZER_FIRE_SOUND.contains("Artillery"));

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.orbit_fields()[0].id;
        let spawn = reg.orbit_fields()[0].spawn_frame;

        // First howitzer tick spawns SpectreHowitzerShell residual honesty.
        reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.howitzer_ticks, 1);
            assert_eq!(f.howitzer_shells_spawned, 1);
            assert_eq!(f.howitzer_shell_fire_fx, 1);
            assert_eq!(f.howitzer_shell_detonation_fx, 1);
            assert_eq!(f.howitzer_shell_height_die_delays, 1);
            assert_eq!(f.howitzer_shell_fire_sounds, 1);
            assert_eq!(f.howitzer_shell_dumb_projectile_applications, 1);
            assert_eq!(f.howitzer_shell_physics_mass_applications, 1);
            assert_eq!(f.howitzer_shell_death_detonated_applications, 1);
            assert_eq!(f.howitzer_shell_death_lasered_applications, 1);
            assert_eq!(f.howitzer_shell_only_moving_down_applications, 1);
        }
        assert!(reg.honesty_howitzer_shell_ok());
        assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
        assert!(reg.honesty_howitzer_ok());

        // Second howitzer residual tick accumulates shell counters.
        let next = spawn + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES;
        reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, next);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.howitzer_ticks, 2);
            assert_eq!(f.howitzer_shells_spawned, 2);
            assert_eq!(f.howitzer_shell_fire_fx, 2);
            assert_eq!(f.howitzer_shell_detonation_fx, 2);
            assert_eq!(f.howitzer_shell_dumb_projectile_applications, 2);
        }
        assert!(reg.honesty_howitzer_shell_ok());
        assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
    }

    #[test]
    fn particle_uplink_outer_beam_width_retail_radius_residual_honesty() {
        // Retail getLaserTemplateWidth = OuterBeamWidth * 0.5 = 13.
        // getCurrentLaserRadius = template * width_scalar.
        // damageRadius = laserRadius * DamageRadiusScalar → peak 44.2.
        // Host combat residual still uses PARTICLE_BEAM_RADIUS 50 × scalar.
        assert!((PARTICLE_ORBITAL_LASER_OUTER_BEAM_WIDTH - 26.0).abs() < 0.01);
        assert!((particle_orbital_laser_template_width() - 13.0).abs() < 0.01);
        assert!((particle_retail_damage_radius(0, 60) - 44.2).abs() < 0.05);
        assert!((PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH - 2.0).abs() < 0.01);
        assert_eq!(PARTICLE_ORBITAL_LASER_NUM_BEAMS, 12);
        assert_eq!(PARTICLE_ORBITAL_LASER_TEXTURE, "EXNoise02.tga");

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let click = Vec3::new(0.0, 0.0, 0.0);
        let id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::China,
            click,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.beam_fields()[0].id;
        let spawn = reg.beam_fields()[0].spawn_frame;
        {
            let f = &reg.beam_fields()[0];
            assert_eq!(f.orbital_laser_draw_params_armed, 1);
            assert_eq!(f.connector_outer_beam_width_armed, 1);
            assert_eq!(f.ground_to_orbit_laser_created, 1);
        }

        // Half WidthGrow: draw width 13, laser r 6.5, retail damage 22.1.
        let half = spawn + PARTICLE_WIDTH_GROW_FRAMES / 2;
        reg.sample_beam_width_honesty(half);
        {
            let f = &reg.beam_fields()[0];
            assert!((f.last_outer_beam_draw_width - 13.0).abs() < 0.1);
            assert!((f.last_retail_laser_radius - 6.5).abs() < 0.1);
            assert!((f.last_retail_damage_radius - 22.1).abs() < 0.1);
            // Host combat radius residual still PARTICLE_BEAM_RADIUS × 0.5 = 25.
            assert!((particle_beam_damage_radius(spawn, half) - 25.0).abs() < 0.1);
        }

        // Full hold: draw 26, laser 13, retail damage 44.2 (host combat 50).
        let hold = spawn + PARTICLE_WIDTH_GROW_FRAMES;
        reg.sample_beam_width_honesty(hold);
        {
            let f = &reg.beam_fields()[0];
            assert!((f.peak_outer_beam_draw_width - 26.0).abs() < 0.1);
            assert!((f.peak_retail_laser_radius - 13.0).abs() < 0.1);
            assert!((f.peak_retail_damage_radius - 44.2).abs() < 0.1);
            assert!((f.last_outer_beam_draw_width - 26.0).abs() < 0.1);
            assert!((particle_beam_damage_radius(spawn, hold) - 50.0).abs() < 0.1);
        }
        assert!(reg.honesty_beam_outer_beam_width_ok());

        // Decay half: draw width 13 again (scalar 0.5).
        let decay_start = particle_decay_start_frame(spawn);
        let half_decay = decay_start + PARTICLE_WIDTH_GROW_FRAMES / 2;
        reg.sample_beam_width_honesty(half_decay);
        {
            let f = &reg.beam_fields()[0];
            assert!((f.last_outer_beam_draw_width - 13.0).abs() < 0.1);
            assert!((f.last_retail_damage_radius - 22.1).abs() < 0.1);
            // Peak hold values preserved.
            assert!((f.peak_retail_damage_radius - 44.2).abs() < 0.1);
        }
        assert!(reg.honesty_beam_outer_beam_width_ok());
        let _ = field_id;
    }

    #[test]
    fn scud_storm_missile_loft_residual_honesty() {
        // Retail ScudStormMissile MissileAIUpdate / HeightDie / Locomotor residual.
        assert_eq!(SCUD_STORM_MISSILE_OBJECT, "ScudStormMissile");
        assert!(!SCUD_STORM_MISSILE_TRY_FOLLOW_TARGET);
        assert_eq!(SCUD_STORM_MISSILE_FUEL_LIFETIME, 0);
        assert!((SCUD_STORM_MISSILE_INITIAL_VELOCITY - 0.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING - 500.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING - 200.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET - 15.0).abs() < 0.01);
        assert_eq!(SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES, 30);
        assert!((SCUD_STORM_MISSILE_LOCOMOTOR_SPEED - 300.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_PREFERRED_HEIGHT - 240.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_PREFERRED_HEIGHT_DAMPING - 0.7).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_GEOMETRY_RADIUS - 7.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_GEOMETRY_HEIGHT - 30.0).abs() < 0.01);
        assert_eq!(SCUD_STORM_MISSILE_IGNITION_FX, "FX_ScudStormIgnition");
        assert_eq!(SCUD_STORM_MISSILE_LAUNCH_SOUND, "ScudStormLaunch");
        assert_eq!(SCUD_STORM_MISSILE_EXHAUST, "ScudMissileExhaust");
        assert_eq!(SCUD_STORM_MISSILE_SPECIAL_POWER, "SuperweaponScudStorm");

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::ScudStorm,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.scud_missile_loft_applications, 0);
            assert!(s.scud_pre_attack_active);
        }
        assert!(!reg.honesty_scud_missile_loft_ok());

        // First missile wave: loft residual + IgnitionFX + HeightDie honesty.
        reg.record_impact_wave(
            id,
            0.0,
            0,
            0,
            1,
            false,
            &[Vec3::new(100.0, 0.0, 100.0)],
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.scud_missile_loft_applications, 1);
            assert_eq!(s.scud_ignition_fx_applications, 1);
            assert_eq!(s.scud_launch_sound_applications, 1);
            assert_eq!(s.scud_exhaust_applications, 1);
            assert_eq!(s.scud_height_die_applications, 1);
            assert_eq!(s.scud_special_power_completion_applications, 1);
            assert!(s.scud_fire_fx_applications >= 1);
            assert!(!s.scud_pre_attack_active);
        }
        assert!(reg.honesty_scud_missile_loft_ok());
        assert!(reg.honesty_scud_pre_attack_and_chem_fx_ok());

        // Second wave accumulates loft residual.
        reg.record_impact_wave(
            id,
            0.0,
            0,
            0,
            1,
            false,
            &[Vec3::new(110.0, 0.0, 90.0)],
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.scud_missile_loft_applications, 2);
            assert_eq!(s.scud_ignition_fx_applications, 2);
            assert_eq!(s.scud_height_die_applications, 2);
        }
        assert!(reg.honesty_scud_missile_loft_ok());
    }

    #[test]
    fn once_at_queue_multi_strike_ocl_residual_honesty() {
        // ArtilleryBarrage: FormationSize Level1 (12) once-at-queue residual.
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(50.0, 0.0, 50.0);
        let id = reg.queue(
            HostSuperweaponKind::ArtilleryBarrage,
            ObjectId(1),
            Team::China,
            target,
            0,
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.ocl_once_at_queue_armed, 1);
            assert_eq!(s.ocl_points.len(), 12);
            assert_eq!(s.ocl_shell_frames.len(), 12);
            // Formation index 0 is spot-on at click target.
            assert!((s.ocl_points[0].x - target.x).abs() < 0.01);
            assert!((s.ocl_points[0].z - target.z).abs() < 0.01);
            // First shell impact matches strike impact_frame residual.
            assert_eq!(s.ocl_shell_frames[0], s.impact_frame);
            // Stored plan matches pure ADC re-query (once-at-queue = index seed).
            let expected = artillery_barrage_points(target);
            assert_eq!(s.ocl_points.len(), expected.len());
            for (a, b) in s.ocl_points.iter().zip(expected.iter()) {
                assert!((a.x - b.x).abs() < 0.01);
                assert!((a.z - b.z).abs() < 0.01);
            }
        }
        assert!(reg.honesty_once_at_queue_ocl_ok());

        // CarpetBomb once-at-queue residual.
        let carpet_id = reg.queue(
            HostSuperweaponKind::CarpetBomb,
            ObjectId(2),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            10,
        );
        {
            let s = reg.get(carpet_id).unwrap();
            assert_eq!(s.ocl_once_at_queue_armed, 1);
            assert_eq!(s.ocl_points.len() as u32, CARPET_BOMB_COUNT);
            assert_eq!(s.ocl_shell_frames.len() as u32, CARPET_BOMB_COUNT);
        }
        assert!(reg.honesty_once_at_queue_ocl_ok());

        // ScudStorm once-at-queue residual (ClipSize 9).
        let scud_id = reg.queue(
            HostSuperweaponKind::ScudStorm,
            ObjectId(3),
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        {
            let s = reg.get(scud_id).unwrap();
            assert_eq!(s.ocl_once_at_queue_armed, 1);
            assert_eq!(s.ocl_points.len() as u32, SCUD_STORM_MISSILE_COUNT);
            assert_eq!(s.ocl_shell_frames.len() as u32, SCUD_STORM_MISSILE_COUNT);
        }

        // One-shot kinds do not arm OCL residual.
        let nuke_id = reg.queue(
            HostSuperweaponKind::NuclearMissile,
            ObjectId(4),
            Team::China,
            Vec3::ZERO,
            0,
        );
        {
            let s = reg.get(nuke_id).unwrap();
            assert_eq!(s.ocl_once_at_queue_armed, 0);
            assert!(s.ocl_points.is_empty());
        }

        // plan_due uses stored ocl_points (Artillery first shell at impact_frame).
        let objects = vec![(ObjectId(99), target, Team::USA, true)];
        let plans = reg.plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, &objects);
        assert!(!plans.is_empty());
        let plan = plans.iter().find(|p| p.strike_id == id).unwrap();
        assert_eq!(plan.wave_shell_count, 1);
        assert!((plan.epicenters[0].x - target.x).abs() < 0.01);
    }

    #[test]
    fn scud_preferred_height_spring_residual_honesty() {
        assert!((scud_missile_spawn_height() - 240.0).abs() < 0.01);
        assert!((scud_missile_preferred_height_spring(0.0) - 168.0).abs() < 0.01);
        assert!((scud_missile_preferred_height_spring(240.0) - 240.0).abs() < 0.01);
        // Multi-frame spring converges toward PreferredHeight.
        let after_10 = scud_missile_preferred_height_after_frames(0.0, 10);
        assert!(after_10 > 168.0);
        assert!(after_10 < 240.0);
        let after_30 = scud_missile_preferred_height_after_frames(0.0, 30);
        assert!(after_30 > after_10);
        assert!(after_30 < 240.0 + 0.01);
        // Phase residual matrix.
        assert_eq!(
            scud_missile_loft_phase(0.0, 1000.0, 100.0),
            ScudMissileLoftPhase::Loft
        );
        assert_eq!(
            scud_missile_loft_phase(500.0, 1000.0, 200.0),
            ScudMissileLoftPhase::Turn
        );
        assert_eq!(
            scud_missile_loft_phase(600.0, 100.0, 100.0),
            ScudMissileLoftPhase::Dive
        );
        assert_eq!(
            scud_missile_loft_phase(600.0, 50.0, 10.0),
            ScudMissileLoftPhase::HeightDie
        );

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::ScudStorm,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(!reg.honesty_scud_preferred_height_spring_ok());
        reg.record_impact_wave(
            id,
            0.0,
            0,
            0,
            1,
            false,
            &[Vec3::new(100.0, 0.0, 100.0)],
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.scud_spawn_height_applications, 1);
            assert_eq!(s.scud_preferred_height_spring_applications, 1);
            assert_eq!(s.scud_loft_phase_peak, ScudMissileLoftPhase::HeightDie);
            assert!(s.scud_last_spring_height > 0.0);
            // 30 spring steps from 0 with damping 0.7: 1 - 0.7^30 of the way to 240.
            let expected = scud_missile_preferred_height_after_frames(0.0, 30);
            assert!((s.scud_last_spring_height - expected).abs() < 0.01);
        }
        assert!(reg.honesty_scud_preferred_height_spring_ok());
        assert!(reg.honesty_scud_missile_loft_ok());
        assert!(reg.honesty_once_at_queue_ocl_ok());
    }

    #[test]
    fn particle_uplink_num_beams_scroll_residual_honesty() {
        assert_eq!(particle_orbital_laser_num_beams(), 12);
        assert!((particle_orbital_laser_tiling_scalar() - 0.15).abs() < 0.01);
        assert!((PARTICLE_ORBITAL_LASER_SCROLL_RATE + 1.75).abs() < 0.01);
        // ScrollRate * (30/30) = -1.75 after one second.
        assert!((particle_orbital_laser_scroll_uv(0, 30) + 1.75).abs() < 0.01);
        assert!((particle_orbital_laser_scroll_uv(100, 100) - 0.0).abs() < 0.01);
        // Two seconds → -3.5.
        assert!((particle_orbital_laser_scroll_uv(0, 60) + 3.5).abs() < 0.01);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let strike_id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        // Spawn beam at impact residual (STATUS_FIRING).
        let field_id = reg.spawn_beam_field(
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            120,
            strike_id,
        );
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.num_beams_armed, 12);
            assert_eq!(f.tiling_scalar_armed, 1);
            assert_eq!(f.scroll_uv_samples, 0);
        }
        assert!(!reg.honesty_beam_num_beams_scroll_ok());

        // Sample width honesty advances scroll UV residual.
        reg.sample_beam_width_honesty(150); // 30 frames after spawn
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.scroll_uv_samples, 1);
            assert!((f.last_scroll_uv + 1.75).abs() < 0.01);
            assert!((f.peak_abs_scroll_uv - 1.75).abs() < 0.01);
        }
        assert!(reg.honesty_beam_num_beams_scroll_ok());

        // Further samples accumulate scroll residual.
        reg.sample_beam_width_honesty(180); // 60 frames after spawn
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.scroll_uv_samples, 2);
            assert!((f.last_scroll_uv + 3.5).abs() < 0.01);
            assert!((f.peak_abs_scroll_uv - 3.5).abs() < 0.01);
        }
        assert!(reg.honesty_beam_num_beams_scroll_ok());
    }

    #[test]
    fn particle_uplink_soft_edge_residual_honesty() {
        // Soft-edge scale: index 0 → 0, index 11 → 1.
        assert!((particle_orbital_soft_edge_scale(0) - 0.0).abs() < 0.01);
        assert!((particle_orbital_soft_edge_scale(11) - 1.0).abs() < 0.01);
        // Mid beam (index 5.5 → 5 / 11 ≈ 0.4545).
        assert!((particle_orbital_soft_edge_scale(5) - 5.0 / 11.0).abs() < 0.01);
        // Outer peak width at full scalar = OuterBeamWidth 26.
        assert!((particle_orbital_soft_edge_outer_width_peak() - 26.0).abs() < 0.01);
        // Inner peak width at full scalar = InnerBeamWidth 0.6.
        assert!(
            (particle_orbital_soft_edge_width(0, 0, PARTICLE_WIDTH_GROW_FRAMES) - 0.6).abs() < 0.01
        );
        // Alpha lerp: inner 250/255 → outer 150/255.
        assert!(
            (particle_orbital_soft_edge_alpha(0) - PARTICLE_ORBITAL_LASER_INNER_COLOR.3).abs()
                < 0.01
        );
        assert!(
            (particle_orbital_soft_edge_alpha(11) - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs()
                < 0.01
        );
        // Color residual: inner white hot → outer blue cool.
        let (ir, ig, ib, _) = particle_orbital_soft_edge_color(0);
        assert!((ir - 1.0).abs() < 0.01 && (ig - 1.0).abs() < 0.01 && (ib - 1.0).abs() < 0.01);
        let (or, og, ob, _) = particle_orbital_soft_edge_color(11);
        assert!((or - 0.0).abs() < 0.01 && (og - 0.0).abs() < 0.01 && (ob - 1.0).abs() < 0.01);
        // Tile factor residual for unit length outer cylinder at full width.
        let tile = particle_orbital_soft_edge_tile_factor(1.0, 26.0);
        assert!((tile - (1.0 / 26.0) * 1.0 * 0.15).abs() < 0.001);
        assert!(PARTICLE_ORBITAL_LASER_TILE);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let strike_id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        let field_id = reg.spawn_beam_field(
            ObjectId(1),
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            120,
            strike_id,
        );
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.soft_edge_color_armed, 1);
            assert_eq!(f.soft_edge_samples, 0);
        }
        assert!(!reg.honesty_beam_soft_edge_ok());

        // Hold frame: full width soft-edge outer residual.
        reg.sample_beam_width_honesty(120 + PARTICLE_WIDTH_GROW_FRAMES);
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.soft_edge_samples, 1);
            assert!((f.peak_soft_edge_outer_width - 26.0).abs() < 0.1);
            assert!((f.last_soft_edge_outer_alpha - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs() < 0.01);
            assert!(f.last_soft_edge_tile_factor > 0.0);
        }
        assert!(reg.honesty_beam_soft_edge_ok());
        assert!(reg.honesty_beam_num_beams_scroll_ok());
    }

    #[test]
    fn particle_uplink_outer_node_bone_layout_residual_honesty() {
        assert_eq!(particle_outer_node_bone_name(0), "FX01");
        assert_eq!(particle_outer_node_bone_name(4), "FX05");
        assert_eq!(PARTICLE_CONNECTOR_BONE_NAME, "FXConnector");
        assert_eq!(PARTICLE_FIRE_BONE_NAME, "FXMain");
        assert_eq!(PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS, 5);
        assert_eq!(PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS, 4);
        assert_eq!(PARTICLE_CONNECTOR_LASER_TEXTURE, "EXLaser.tga");

        let origin = Vec3::new(10.0, 0.0, 20.0);
        let p0 = particle_outer_node_bone_position(origin, 0);
        // FX01 at angle 0: +radius on X, height on Y.
        assert!((p0.x - (origin.x + PARTICLE_OUTER_NODE_RING_RADIUS)).abs() < 0.01);
        assert!((p0.y - PARTICLE_OUTER_NODE_RING_HEIGHT).abs() < 0.01);
        assert!((p0.z - origin.z).abs() < 0.01);
        let p1 = particle_outer_node_bone_position(origin, 1);
        // 72 degrees around ring.
        assert!((p1.y - PARTICLE_OUTER_NODE_RING_HEIGHT).abs() < 0.01);
        assert!((p1.x - origin.x).abs() > 1.0 || (p1.z - origin.z).abs() > 1.0);
        let conn = particle_connector_bone_position(origin);
        assert!((conn.x - origin.x).abs() < 0.01);
        assert!((conn.y - PARTICLE_OUTER_NODE_RING_HEIGHT).abs() < 0.01);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let strike_id = reg.queue(
            HostSuperweaponKind::ParticleCannon,
            ObjectId(1),
            Team::USA,
            origin,
            0,
        );
        let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, origin, 120, strike_id);
        {
            let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
            assert_eq!(f.outer_node_bone_layout_applications, 5);
            assert_eq!(f.connector_bone_layout_applications, 1);
            assert!((f.last_outer_node_bone_position.x
                - (origin.x + PARTICLE_OUTER_NODE_RING_RADIUS))
                .abs()
                < 0.01);
        }
        assert!(reg.honesty_beam_outer_node_bone_layout_ok());
        assert!(reg.honesty_beam_outer_nodes_ok());
        let _ = field_id;
    }

    #[test]
    fn scud_ballistic_flight_residual_honesty() {
        assert_eq!(SCUD_STORM_MISSILE_MODEL, "UBScudStrm_M");
        assert!(SCUD_STORM_MISSILE_HEIGHT_DIE_ONLY_MOVING_DOWN);
        assert!(SCUD_STORM_MISSILE_SNAP_TO_GROUND_ON_DEATH);
        assert!(SCUD_STORM_MISSILE_HEIGHT_DIE_INCLUDES_STRUCTURES);
        assert!((SCUD_STORM_MISSILE_LOCOMOTOR_ACCEL - 675.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_LOCOMOTOR_TURN_RATE - 540.0).abs() < 0.01);
        assert!((SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01);
        assert!(SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL);
        assert!((scud_missile_speed_per_frame() - 10.0).abs() < 0.01);

        // Ballistic sample over enough frames to reach HeightDie residual.
        let launch = Vec3::new(0.0, 0.0, 0.0);
        let target = Vec3::new(700.0, 0.0, 0.0);
        let (pos, traveled, dist_to, phase) =
            scud_missile_ballistic_sample(launch, target, 120);
        assert!(traveled > 0.0);
        assert!(phase == ScudMissileLoftPhase::HeightDie || dist_to < 200.0 || pos.y <= 15.0);
        // After HeightDie snap, Y is surface.
        if phase == ScudMissileLoftPhase::HeightDie {
            assert!((pos.y - 0.0).abs() < 0.01);
        }

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::ScudStorm,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(!reg.honesty_scud_ballistic_flight_ok());
        reg.record_impact_wave(
            id,
            0.0,
            0,
            0,
            1,
            false,
            &[Vec3::new(100.0, 0.0, 100.0)],
        );
        {
            let s = reg.get(id).unwrap();
            assert_eq!(s.scud_ballistic_flight_applications, 1);
            assert_eq!(s.scud_only_moving_down_applications, 1);
            assert_eq!(s.scud_snap_to_ground_applications, 1);
            assert_eq!(s.scud_model_draw_applications, 1);
            assert!(s.scud_peak_flight_distance > 0.0);
            assert_eq!(s.scud_loft_phase_peak, ScudMissileLoftPhase::HeightDie);
        }
        assert!(reg.honesty_scud_ballistic_flight_ok());
        assert!(reg.honesty_scud_preferred_height_spring_ok());
        assert!(reg.honesty_scud_missile_loft_ok());
    }

    #[test]
    fn spectre_howitzer_shell_model_draw_residual_honesty() {
        assert_eq!(SPECTRE_HOWITZER_SHELL_MODEL, "AVSpectreShell1");
        assert!((SPECTRE_HOWITZER_SHELL_SCALE - 0.6).abs() < 0.01);
        assert_eq!(SPECTRE_HOWITZER_SHELL_SHADOW, "SHADOW_DECAL");
        assert_eq!(SPECTRE_HOWITZER_SHELL_GEOMETRY, "Cylinder");
        assert!(SPECTRE_HOWITZER_SHELL_GEOMETRY_IS_SMALL);
        assert!((SPECTRE_HOWITZER_SHELL_MAX_HEALTH - 100.0).abs() < 0.01);

        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::SpectreGunship,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        reg.record_impact_complete(id, 0.0, 0, 0);
        let field_id = reg.orbit_fields()[0].id;
        let spawn = reg.orbit_fields()[0].spawn_frame;
        assert!(!reg.honesty_howitzer_shell_model_draw_ok());
        // One howitzer tick residual.
        reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn);
        {
            let f = &reg.orbit_fields()[0];
            assert_eq!(f.howitzer_shell_model_draw_applications, 1);
            assert_eq!(f.howitzer_shell_scale_applications, 1);
            assert_eq!(f.howitzer_shell_shadow_applications, 1);
            assert_eq!(f.howitzer_shell_geometry_applications, 1);
            assert_eq!(f.howitzer_shell_max_health_applications, 1);
        }
        assert!(reg.honesty_howitzer_shell_model_draw_ok());
        assert!(reg.honesty_howitzer_shell_ok());
        assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
    }
}
