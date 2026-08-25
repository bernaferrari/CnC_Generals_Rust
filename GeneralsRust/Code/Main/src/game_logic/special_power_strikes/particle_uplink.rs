//! Particle Uplink status, intensity, beam, remnant, and client residual helpers.
use super::types::*;
// --- Particle Uplink continuous beam residual (ParticleUplinkCannonUpdate) ---

/// Retail `ParticleUplinkCannonUpdate` TotalFiringTime = 3500 ms → 105 frames @ 30 FPS.
pub const PARTICLE_BEAM_DURATION_FRAMES: u32 = 105;
/// Retail TotalDamagePulses = 40.
pub const PARTICLE_BEAM_TOTAL_PULSES: u32 = 40;
/// Retail DamagePerSecond = 400.
/// damagePerPulse = (TotalFiringFrames/FPS * DamagePerSecond) / TotalDamagePulses
///                 = (105/30 * 400) / 40 = 35.
pub const PARTICLE_BEAM_DAMAGE_PER_PULSE: f32 = 35.0;
/// Retail FactionBuilding.ini `ParticleUplinkCannonUpdate` DamageType residual.
/// C++ ctor defaults to DAMAGE_LASER; authored INI is PARTICLE_BEAM.
pub const PARTICLE_BEAM_DAMAGE_TYPE: &str = "PARTICLE_BEAM";
/// Retail `ParticleUplinkCannonUpdate` DeathType residual (DEATH_LASERED).
pub const PARTICLE_BEAM_DEATH_TYPE: &str = "LASERED";

/// Residual pulse interval floor: TotalFiringTime / TotalDamagePulses → 105/40
/// ≈ 2.625 frames. Host residual prefers fractional nextFactor scheduling
/// ([`particle_next_pulse_frame`]); this constant remains the minimum gap honesty.
pub const PARTICLE_BEAM_TICK_INTERVAL_FRAMES: u32 = 3;
/// Retail peak damage radius at target: OuterBeamWidth×0.5 × DamageRadiusScalar
/// = 13 × 3.4 = **44.2**. WidthGrow scales this 0→peak→0.
pub const PARTICLE_BEAM_RADIUS: f32 = 44.2;
/// Retail `ParticleUplinkCannonUpdate` DamageRadiusScalar = 3.4.
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
/// peak damage = 13 × 3.4 = **44.2** = [`PARTICLE_BEAM_RADIUS`].
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
pub const PARTICLE_ORBITAL_LASER_INNER_COLOR: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 250.0 / 255.0);
/// Retail OrbitalLaser OuterColor residual (R:0 G:0 B:255 A:150).
pub const PARTICLE_ORBITAL_LASER_OUTER_COLOR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail OrbitalLaser Tile residual (`Tile = Yes`).
pub const PARTICLE_ORBITAL_LASER_TILE: bool = true;
/// Host residual texture aspect for tile-factor honesty (fail-closed vs live surface desc).
pub const PARTICLE_ORBITAL_LASER_TEXTURE_ASPECT: f32 = 1.0;
/// Retail OrbitalLaser VisionRange residual (design params).
pub const PARTICLE_ORBITAL_LASER_VISION_RANGE: f32 = 100.0;
/// Retail OrbitalLaser ShroudClearingRange residual (design params).
pub const PARTICLE_ORBITAL_LASER_SHROUD_CLEARING_RANGE: f32 = 120.0;
/// Retail OrbitalLaser KindOf residual.
pub const PARTICLE_ORBITAL_LASER_KIND_OF: &str = "IMMOBILE";
/// Retail W3DLaserDraw Segments residual default (OrbitalLaser omits Segments → 1).
pub const PARTICLE_ORBITAL_LASER_SEGMENTS: u32 = 1;
/// Retail W3DLaserDraw ArcHeight residual default (0 = no arc).
pub const PARTICLE_ORBITAL_LASER_ARC_HEIGHT: f32 = 0.0;
/// Retail W3DLaserDraw SegmentOverlapRatio residual default.
pub const PARTICLE_ORBITAL_LASER_SEGMENT_OVERLAP: f32 = 0.0;
/// Retail LaserUpdate orbit altitude residual (`orbitPosition.z += 500` in C++).
///
/// Host residual uses Y-up (glam); C++ engine Z-up — both track height as +500.
pub const PARTICLE_LASER_ORBIT_ALTITUDE: f32 = 500.0;
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
/// Retail Intense connector InnerBeamWidth residual.
pub const PARTICLE_CONNECTOR_INTENSE_INNER_BEAM_WIDTH: f32 = 0.6;
/// Retail Medium connector InnerBeamWidth residual.
pub const PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH: f32 = 0.4;
/// Retail connector InnerColor residual (R:255 G:255 B:255 A:250).
pub const PARTICLE_CONNECTOR_INNER_COLOR: (f32, f32, f32, f32) = (1.0, 1.0, 1.0, 250.0 / 255.0);
/// Retail connector OuterColor residual (R:0 G:0 B:255 A:150).
pub const PARTICLE_CONNECTOR_OUTER_COLOR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail SupW (superweapon general) connector/orbital OuterColor residual
/// (R:255 G:0 B:255 A:150 magenta vs normal blue).
pub const PARTICLE_SUPW_CONNECTOR_OUTER_COLOR: (f32, f32, f32, f32) =
    (1.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail SupW_ParticleUplinkCannon_OrbitalLaser OuterColor residual (magenta).
pub const PARTICLE_SUPW_ORBITAL_OUTER_COLOR: (f32, f32, f32, f32) = (1.0, 0.0, 1.0, 150.0 / 255.0);
/// Retail SupW medium/intense/orbital object name residual prefixes.
pub const PARTICLE_SUPW_MEDIUM_CONNECTOR: &str = "SupW_ParticleUplinkCannon_MediumConnectorLaser";
pub const PARTICLE_SUPW_INTENSE_CONNECTOR: &str = "SupW_ParticleUplinkCannon_IntenseConnectorLaser";
pub const PARTICLE_SUPW_ORBITAL_LASER: &str = "SupW_ParticleUplinkCannon_OrbitalLaser";
/// Retail connector KindOf residual (Medium/Intense both IMMOBILE).
pub const PARTICLE_CONNECTOR_KIND_OF: &str = "IMMOBILE";
/// Retail W3DLaserDraw Segments residual default (connectors omit Segments → 1).
pub const PARTICLE_CONNECTOR_SEGMENTS: u32 = 1;
/// Retail W3DLaserDraw ArcHeight residual default (connectors omit ArcHeight → 0).
pub const PARTICLE_CONNECTOR_ARC_HEIGHT: f32 = 0.0;
/// Retail W3DLaserDraw SegmentOverlapRatio residual default (connectors omit → 0).
pub const PARTICLE_CONNECTOR_SEGMENT_OVERLAP: f32 = 0.0;
/// Retail W3DLaserDraw MaxIntensityLifetime residual default (connectors omit → 0).
pub const PARTICLE_CONNECTOR_MAX_INTENSITY_FRAMES: u32 = 0;
/// Retail W3DLaserDraw FadeLifetime residual default (connectors omit → 0).
pub const PARTICLE_CONNECTOR_FADE_FRAMES: u32 = 0;
/// Retail connector Tile residual (connectors omit Tile → No).
pub const PARTICLE_CONNECTOR_TILE: bool = false;
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
/// Residual MaxHealth for connector laser ThingFactory Objects (no Body in retail).
pub const PARTICLE_CONNECTOR_LASER_MAX_HEALTH: f32 = 1.0;
/// Retail ConnectorIntenseLaserName (STATUS_FIRING residual).
pub const PARTICLE_CONNECTOR_INTENSE_LASER: &str = "ParticleUplinkCannon_IntenseConnectorLaser";
/// Retail LaserBaseLightFlareParticleSystemName (ready residual honesty).
pub const PARTICLE_LASER_BASE_READY_FLARE: &str = "ParticleUplinkCannon_LaserBaseReadyToFire";
/// Retail ParticleBeamLaserName (ground↔orbit + orbit→target lasers).
pub const PARTICLE_ORBITAL_LASER_NAME: &str = "ParticleUplinkCannon_OrbitalLaser";
/// Residual MaxHealth for OrbitalLaser ThingFactory Object (no Body module in retail).
pub const PARTICLE_ORBITAL_LASER_MAX_HEALTH: f32 = 1.0;
/// Retail commented ConnectorMediumFlare residual name (FactionBuilding.ini).
///
/// Present as residual name table honesty only — retail block is commented out;
/// host does **not** claim live ParticleSystemManager connector-flare spawn.
pub const PARTICLE_CONNECTOR_MEDIUM_FLARE: &str = "ParticleUplinkCannon_InnerConnectorMediumFlare";
/// Retail commented ConnectorIntenseFlare residual name.
pub const PARTICLE_CONNECTOR_INTENSE_FLARE: &str =
    "ParticleUplinkCannon_InnerConnectorIntenseFlare";

/// Wave 81 residual name table: intensity key → outer-node flare particle system.
///
/// Fail-closed residual pack (not live FX spawn). Order: Light → Medium → Intense.
pub const PARTICLE_OUTER_NODE_FLARE_NAME_TABLE: &[(&str, &str)] = &[
    ("Light", PARTICLE_OUTER_NODE_LIGHT_FLARE),
    ("Medium", PARTICLE_OUTER_NODE_MEDIUM_FLARE),
    ("Intense", PARTICLE_OUTER_NODE_INTENSE_FLARE),
];

/// Wave 81 residual name table: ready / connector / orbital particle system names.
pub const PARTICLE_UPLINK_FLARE_LASER_NAME_TABLE: &[(&str, &str)] = &[
    ("LaserBaseReady", PARTICLE_LASER_BASE_READY_FLARE),
    ("ConnectorMediumLaser", PARTICLE_CONNECTOR_MEDIUM_LASER),
    ("ConnectorIntenseLaser", PARTICLE_CONNECTOR_INTENSE_LASER),
    ("OrbitalLaser", PARTICLE_ORBITAL_LASER_NAME),
    ("ConnectorMediumFlare", PARTICLE_CONNECTOR_MEDIUM_FLARE),
    ("ConnectorIntenseFlare", PARTICLE_CONNECTOR_INTENSE_FLARE),
];
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
/// C++ first damage at orbital birth (`startAttack + BeamTravelTime`).
/// Host `impact_delay_frames` is this travel residual (not a charge subset).
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

// --- SupW PointDefenseDroneLaserBeam / PointDefenseLaserBeam LifetimeUpdate ---
// Retail WeaponObjects.ini / SuperWeaponGeneral.ini: MinLifetime=MaxLifetime=95 ms.

/// Retail SupW_PointDefenseDroneLaserBeam object name residual.
pub const POINT_DEFENSE_DRONE_LASER_BEAM: &str = "SupW_PointDefenseDroneLaserBeam";
/// Retail PointDefenseLaserBeam object name residual (same LifetimeUpdate).
pub const POINT_DEFENSE_LASER_BEAM: &str = "PointDefenseLaserBeam";
/// Retail LifetimeUpdate MinLifetime residual (msec).
pub const POINT_DEFENSE_LASER_MIN_LIFETIME_MS: u32 = 95;
/// Retail LifetimeUpdate MaxLifetime residual (msec; equals Min for fixed life).
pub const POINT_DEFENSE_LASER_MAX_LIFETIME_MS: u32 = 95;
/// LifetimeUpdate Min==Max 95 ms → [`duration_ms_to_logic_frames`] = **3** frames.
///
/// C++ `ConvertDurationFromMsecsToFrames` = ceil(msec * 30 / 1000):
/// ceil(95*30/1000) = ceil(2.85) = 3. Fail-closed: not full LifetimeUpdate
/// destroyObject on dieFrame / ThingFactory laser drawable.
pub const POINT_DEFENSE_LASER_LIFETIME_FRAMES: u32 = (95 * 30 + 999) / 1000;

// --- AmericaParticleUplinkCannon FlammableUpdate residual ---
// Retail FactionBuilding.ini ModuleTag_14 on Particle Uplink building.

/// Retail FlammableUpdate AflameDuration residual (msec).
pub const PARTICLE_UPLINK_AFLAME_DURATION_MS: u32 = 5000;
/// AflameDuration 5000 ms → 150 frames @ 30 FPS.
pub const PARTICLE_UPLINK_AFLAME_DURATION_FRAMES: u32 = (5000 * 30) / 1000;
/// Retail FlammableUpdate AflameDamageAmount residual.
pub const PARTICLE_UPLINK_AFLAME_DAMAGE_AMOUNT: f32 = 5.0;
/// Retail FlammableUpdate AflameDamageDelay residual (msec).
pub const PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_MS: u32 = 500;
/// AflameDamageDelay 500 ms → 15 frames @ 30 FPS.
pub const PARTICLE_UPLINK_AFLAME_DAMAGE_DELAY_FRAMES: u32 = (500 * 30) / 1000;

// --- AmericaParticleUplinkCannon SlowDeath / InstantDeath residual ---
// Retail FactionBuilding.ini ModuleTag_18 / ModuleTag_19 on complete vs
// under-construction building death paths.

/// Retail SlowDeathBehavior ExemptStatus residual (skip when under construction).
pub const PARTICLE_UPLINK_SLOW_DEATH_EXEMPT_STATUS: &str = "UNDER_CONSTRUCTION";
/// Retail SlowDeathBehavior DestructionDelay residual (msec).
pub const PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_MS: u32 = 2000;
/// DestructionDelay 2000 ms → 60 frames @ 30 FPS.
pub const PARTICLE_UPLINK_SLOW_DEATH_DESTRUCTION_DELAY_FRAMES: u32 = (2000 * 30) / 1000;
/// Retail SlowDeath INITIAL FX residual.
pub const PARTICLE_UPLINK_SLOW_DEATH_FX_INITIAL: &str = "FX_ParticleUplinkDeathInitial";
/// Retail SlowDeath INITIAL OCL residual.
pub const PARTICLE_UPLINK_SLOW_DEATH_OCL_INITIAL: &str = "OCL_SDILinkLasers";
/// Retail SlowDeath FINAL FX residual.
pub const PARTICLE_UPLINK_SLOW_DEATH_FX_FINAL: &str = "FX_StructureMediumDeath";
/// Retail SlowDeath FINAL OCL residual.
pub const PARTICLE_UPLINK_SLOW_DEATH_OCL_FINAL: &str = "OCL_ParticleUplinkDeathFinal";
/// Retail InstantDeath RequiredStatus residual (under construction only).
pub const PARTICLE_UPLINK_INSTANT_DEATH_REQUIRED_STATUS: &str = "UNDER_CONSTRUCTION";
/// Retail InstantDeath OCL residual (under construction explode).
pub const PARTICLE_UPLINK_INSTANT_DEATH_OCL: &str = "OCL_ABPowerPlantExplode";
/// Retail InstantDeath FX residual.
pub const PARTICLE_UPLINK_INSTANT_DEATH_FX: &str = "FX_StructureMediumDeath";
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
    let offset = (factor * (PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES as f32)).floor() as u32;
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
/// Host residual uses pulse index as time factor. Local cartesian lives in the
/// host x/z plane (`C++ x → host x`, `C++ y → host z`). Callers that know the
/// cannon position must rotate via [`particle_swath_offset_along`] /
/// [`particle_swath_epicenter_along`] (leftover already matches C++).
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

/// Rotate local SwathOfDeath offset onto the building→target ground axis.
///
/// Leftover `ParticleUplinkCannonUpdate` already matches C++: S-curve is
/// authored with the click on +X from the cannon, then rotated onto
/// `building → initialTarget` (`x' = x nx − y ny`, `y' = x ny + y nx`).
/// Degenerate axis (cannon on the click) keeps world +X.
pub fn particle_swath_offset_along(
    pulses_made_before_this_pulse: u32,
    building: Vec3,
    target: Vec3,
) -> Vec3 {
    let local = particle_swath_offset(pulses_made_before_this_pulse);
    let dx = target.x - building.x;
    let dz = target.z - building.z;
    let len_sq = dx * dx + dz * dz;
    if len_sq <= 1.0e-4 {
        return local;
    }
    let inv = 1.0 / len_sq.sqrt();
    let nx = dx * inv;
    let nz = dz * inv;
    Vec3::new(
        local.x * nx - local.z * nz,
        0.0,
        local.x * nz + local.z * nx,
    )
}

/// Absolute residual damage epicenter for a pulse at field spawn position.
/// World-axis offset; prefer [`particle_swath_epicenter_along`] when the cannon
/// position is known.
pub fn particle_swath_epicenter(base: Vec3, pulses_made_before_this_pulse: u32) -> Vec3 {
    base + particle_swath_offset(pulses_made_before_this_pulse)
}

/// Absolute SwathOfDeath epicenter rotated onto cannon→click (C++ / leftover).
pub fn particle_swath_epicenter_along(
    building: Vec3,
    target: Vec3,
    pulses_made_before_this_pulse: u32,
) -> Vec3 {
    target + particle_swath_offset_along(pulses_made_before_this_pulse, building, target)
}

/// Absolute frame when WidthGrow decay starts (`LaserUpdate::setDecayFrames`).
///
/// Retail: `orbitalDecayStart = startAttack + totalFiring + beamTravel` relative
/// to orbital birth → `spawn + TotalFiringTime`.
pub fn particle_decay_start_frame(spawn_frame: u32) -> u32 {
    spawn_frame.saturating_add(PARTICLE_BEAM_DURATION_FRAMES)
}

/// C++ `ParticleUplinkCannonUpdate.cpp:407-410` live-beam abort mask.
///
/// UNDERPOWERED / EMP / SUBDUED / HACKED force `m_startDecayFrame = now`.
#[inline]
pub const fn puc_disabled_aborts_live_beam(
    underpowered: bool,
    emp: bool,
    subdued: bool,
    hacked: bool,
) -> bool {
    underpowered || emp || subdued || hacked
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
    if current_frame < spawn_frame {
        return 0.0;
    }
    // Beam-start frame residual: first grow step so spawn-frame damage pulse has
    // non-zero radius (next_tick_frame == spawn_frame; zero width would no-op).
    if current_frame == spawn_frame {
        return (1.0 / (PARTICLE_WIDTH_GROW_FRAMES as f32)).clamp(0.0, 1.0);
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
/// Full radius is [`PARTICLE_BEAM_RADIUS`] (**44.2**) while hold. Early grow
/// and late decay pulses use a smaller radius (retail laser radius × scalar).
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
/// Linear unpremultiplied lerp residual (host honesty / multi-beam pack).
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

/// Soft-edge RGB residual with C++ W3DLaserDraw innerAlpha premultiply on channel delta.
///
/// C++: `red = innerRed + scale * (outerRed - innerRed) * innerAlpha` (same for G/B).
/// Alpha still lerps InnerColor.A → OuterColor.A without extra premultiply.
/// Fail-closed: not full SegLineRenderer additive GPU submit.
#[inline]
pub fn particle_orbital_soft_edge_color_premul(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_orbital_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_ORBITAL_LASER_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_ORBITAL_LASER_OUTER_COLOR;
    (
        ir + scale * (or - ir) * ia,
        ig + scale * (og - ig) * ia,
        ib + scale * (ob - ib) * ia,
        ia + scale * (oa - ia),
    )
}

/// Single-beam RGB residual with C++ W3DLaserDraw NumBeams==1 path.
///
/// C++: when `m_numBeams == 1`, RGB is fully multiplied by innerAlpha
/// (`red = innerRed * innerAlpha`) and alpha = innerAlpha. Fail-closed: not full
/// SegLineRenderer GPU submit (OrbitalLaser uses multi-beam path; this residual
/// tracks the single-beam branch for connector / generic laser honesty).
#[inline]
pub fn particle_orbital_single_beam_color_premul() -> (f32, f32, f32, f32) {
    let (ir, ig, ib, ia) = PARTICLE_ORBITAL_LASER_INNER_COLOR;
    (ir * ia, ig * ia, ib * ia, ia)
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
    (length / width) * PARTICLE_ORBITAL_LASER_TEXTURE_ASPECT * PARTICLE_ORBITAL_LASER_TILING_SCALAR
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

/// C++ `ParticleUplinkCannonUpdate::createEffects` outer-node flares.
///
/// Attaches Light/Medium/Intense templates at FX01..FX05 ring locals so the
/// dish is not visually idle through CHARGING→FIRING.
pub fn spawn_particle_outer_node_flares(
    source: ObjectId,
    building_origin: Vec3,
    intensity: ParticleIntensity,
) {
    let name = intensity.outer_flare_name();
    if name.is_empty() {
        return;
    }
    for i in 0..PARTICLE_OUTER_EFFECT_NUM_BONES {
        let world = particle_outer_node_bone_position(building_origin, i);
        let local = world - building_origin;
        let cpp = gamelogic::common::Coord3D::new(local.x, local.z, local.y);
        let _ = gamelogic::helpers::attach_particle_system_to_object_local(
            name,
            source.0,
            Some(&cpp),
            None,
        );
    }
}

/// C++ `ParticleUplinkCannonUpdate.cpp:711-721` BeamLaunchFX at laser origin.
pub fn play_particle_beam_launch_fx(origin: Vec3) {
    let _ = crate::game_logic::dispatch_fx_list_at_pos(PARTICLE_BEAM_LAUNCH_FX, origin);
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

/// Intense connector soft-edge scale residual (`i / (NumBeams-1)`).
#[inline]
pub fn particle_connector_intense_soft_edge_scale(beam_index: u32) -> f32 {
    if PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS <= 1 {
        return 0.0;
    }
    let i = beam_index.min(PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS - 1) as f32;
    i / (PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS as f32 - 1.0)
}

/// Intense connector soft-edge width residual for beam index.
#[inline]
pub fn particle_connector_intense_soft_edge_width(beam_index: u32) -> f32 {
    let scale = particle_connector_intense_soft_edge_scale(beam_index);
    PARTICLE_CONNECTOR_INTENSE_INNER_BEAM_WIDTH
        + scale
            * (PARTICLE_CONNECTOR_INTENSE_OUTER_BEAM_WIDTH
                - PARTICLE_CONNECTOR_INTENSE_INNER_BEAM_WIDTH)
}

/// Intense connector soft-edge color residual for beam index (linear RGB lerp).
#[inline]
pub fn particle_connector_intense_soft_edge_color(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_connector_intense_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_CONNECTOR_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_CONNECTOR_OUTER_COLOR;
    (
        ir + scale * (or - ir),
        ig + scale * (og - ig),
        ib + scale * (ob - ib),
        ia + scale * (oa - ia),
    )
}

/// Intense connector soft-edge RGB residual with C++ innerAlpha premultiply.
///
/// C++ W3DLaserDraw: `red = inner + scale * (outer - inner) * innerAlpha`.
/// Fail-closed: not full LaserUpdate drawable / GPU SegLine submit.
#[inline]
pub fn particle_connector_intense_soft_edge_color_premul(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_connector_intense_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_CONNECTOR_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_CONNECTOR_OUTER_COLOR;
    (
        ir + scale * (or - ir) * ia,
        ig + scale * (og - ig) * ia,
        ib + scale * (ob - ib) * ia,
        ia + scale * (oa - ia),
    )
}

/// Medium connector soft-edge scale residual (`i / (NumBeams-1)`).
#[inline]
pub fn particle_connector_medium_soft_edge_scale(beam_index: u32) -> f32 {
    if PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS <= 1 {
        return 0.0;
    }
    let i = beam_index.min(PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS - 1) as f32;
    i / (PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS as f32 - 1.0)
}

/// Medium connector soft-edge width residual for beam index.
#[inline]
pub fn particle_connector_medium_soft_edge_width(beam_index: u32) -> f32 {
    let scale = particle_connector_medium_soft_edge_scale(beam_index);
    PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH
        + scale
            * (PARTICLE_CONNECTOR_MEDIUM_OUTER_BEAM_WIDTH
                - PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH)
}

/// Medium connector soft-edge color residual for beam index (linear RGB lerp).
#[inline]
pub fn particle_connector_medium_soft_edge_color(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_connector_medium_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_CONNECTOR_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_CONNECTOR_OUTER_COLOR;
    (
        ir + scale * (or - ir),
        ig + scale * (og - ig),
        ib + scale * (ob - ib),
        ia + scale * (oa - ia),
    )
}

/// Medium connector soft-edge RGB residual with C++ innerAlpha premultiply.
#[inline]
pub fn particle_connector_medium_soft_edge_color_premul(beam_index: u32) -> (f32, f32, f32, f32) {
    let scale = particle_connector_medium_soft_edge_scale(beam_index);
    let (ir, ig, ib, ia) = PARTICLE_CONNECTOR_INNER_COLOR;
    let (or, og, ob, oa) = PARTICLE_CONNECTOR_OUTER_COLOR;
    (
        ir + scale * (or - ir) * ia,
        ig + scale * (og - ig) * ia,
        ib + scale * (ob - ib) * ia,
        ia + scale * (oa - ia),
    )
}

/// Connector laser residual segment endpoints (outer-node bone → connector bone).
///
/// Fail-closed: not full LaserUpdate drawable object / client shroud path.
#[inline]
pub fn particle_connector_laser_segment(
    building_origin: Vec3,
    outer_node_index: u32,
) -> (Vec3, Vec3) {
    (
        particle_outer_node_bone_position(building_origin, outer_node_index),
        particle_connector_bone_position(building_origin),
    )
}

/// Ground-to-orbit LaserUpdate residual segment (`createGroundToOrbitLaser`).
///
/// C++: start = laser origin, end = origin + 500 height. Fail-closed: not live
/// bone extract / drawable ThingFactory Object.
#[inline]
pub fn particle_ground_to_orbit_laser_segment(laser_origin: Vec3) -> (Vec3, Vec3) {
    let end = Vec3::new(
        laser_origin.x,
        laser_origin.y + PARTICLE_LASER_ORBIT_ALTITUDE,
        laser_origin.z,
    );
    (laser_origin, end)
}

/// Orbit-to-target LaserUpdate residual segment (`createOrbitToTargetLaser`).
///
/// C++: start = target + 500 height, end = target position.
#[inline]
pub fn particle_orbit_to_target_laser_segment(target: Vec3) -> (Vec3, Vec3) {
    let start = Vec3::new(target.x, target.y + PARTICLE_LASER_ORBIT_ALTITUDE, target.z);
    (start, target)
}

/// LaserUpdate drawable midpoint residual (`(start+end)*0.5` when no parent).
///
/// C++: `posToUse = (start+end)*0.5` so the laser is not culled off-screen.
#[inline]
pub fn laser_update_drawable_midpoint(start: Vec3, end: Vec3) -> Vec3 {
    (start + end) * 0.5
}

/// LaserUpdate `m_currentWidthScalar` residual while widening (`sizeDeltaFrames > 0`).
///
/// C++: `(now - widenStart) / (widenFinish - widenStart)` clamped to [0,1].
#[inline]
pub fn laser_update_width_scalar_widen(elapsed_frames: u32, growth_frames: u32) -> f32 {
    if growth_frames == 0 {
        return 1.0;
    }
    (elapsed_frames as f32 / growth_frames as f32).clamp(0.0, 1.0)
}

/// LaserUpdate `m_currentWidthScalar` residual while decaying (`setDecayFrames`).
///
/// C++: `1.0 - (now - decayStart) / (decayFinish - decayStart)` clamped to [0,1].
#[inline]
pub fn laser_update_width_scalar_decay(elapsed_frames: u32, decay_frames: u32) -> f32 {
    if decay_frames == 0 {
        return 0.0;
    }
    (1.0 - elapsed_frames as f32 / decay_frames as f32).clamp(0.0, 1.0)
}

/// LaserUpdate `getCurrentLaserRadius` residual = templateWidth × widthScalar.
///
/// Template width residual is OuterBeamWidth × 0.5 (retail peak 13.0 at full scalar).
#[inline]
pub fn laser_update_current_radius(width_scalar: f32) -> f32 {
    particle_orbital_laser_template_width() * width_scalar
}

/// Retail damage-radius formula honesty residual
/// (`getCurrentLaserRadius() * DamageRadiusScalar`).
///
/// Peak hold = 13 × 3.4 = **44.2**. Combat pulses use the same formula via
/// [`particle_beam_damage_radius`].
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

/// C++ `ParticleUplinkCannonUpdate.cpp:603-614`.
///
/// Leftover `ParticleUplinkCannonUpdate` already calls
/// `TheGameClient::add_scorch` + `TheFXListStore` GroundHitFX. Live beams are
/// residual-only (Wave 299 empty dual-world), so replay those leftover client
/// calls from each due scorch event.
///
/// `GameClientRandomValue(SCORCH_1, SCORCH_4)` — C++ `SCORCH_1=0` .. `SCORCH_4=3`.
/// Host `position` is Y-up; leftover / `addScorch` is Z-up.
pub fn apply_particle_beam_scorch_and_ground_hit_fx(position: Vec3, scorch_radius: f32) {
    const SCORCH_1: i32 = 0;
    const SCORCH_4: i32 = 3;
    let leftover = gamelogic::common::Coord3D::new(position.x, position.z, position.y);
    if let Some(client) = gamelogic::helpers::TheGameClient::get() {
        let scorch_id = gamelogic::helpers::game_client_random_value(SCORCH_1, SCORCH_4);
        client.add_scorch(&leftover, scorch_radius, scorch_id);
    }
    let _ = crate::game_logic::dispatch_fx_list_at_pos(PARTICLE_GROUND_HIT_FX, position);
}

/// Next absolute frame for the next scorch mark (fractional residual).
///
/// C++ after each scorch: `nextFactor = scorchMarksMade / totalScorchMarks`,
/// `m_nextScorchMarkFrame = orbitalBirth + nextFactor * orbitalLifetime`.
pub fn particle_next_scorch_frame(spawn_frame: u32, scorch_marks_made: u32) -> u32 {
    if PARTICLE_TOTAL_SCORCH_MARKS == 0 {
        return spawn_frame.saturating_add(PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES);
    }
    let factor = (scorch_marks_made as f32) / (PARTICLE_TOTAL_SCORCH_MARKS as f32);
    let offset = (factor * (PARTICLE_BEAM_ORBITAL_LIFETIME_FRAMES as f32)).floor() as u32;
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
/// Retail DeletionUpdate MinLifetime residual (msec).
pub const PARTICLE_REMNANT_MIN_LIFETIME_MS: u32 = 4000;
/// Retail DeletionUpdate MaxLifetime residual (msec; equals Min for fixed lifetime).
pub const PARTICLE_REMNANT_MAX_LIFETIME_MS: u32 = 4000;
/// Retail DeletionUpdate Min/MaxLifetime 4000 ms → 120 frames.
pub const PARTICLE_REMNANT_DURATION_FRAMES: u32 = (PARTICLE_REMNANT_MIN_LIFETIME_MS * 30) / 1000;
/// Retail remnant Object template name residual (honesty).
pub const PARTICLE_REMNANT_OBJECT_NAME: &str = "ParticleUplinkCannonTrailRemnant";
/// Retail remnant weapon name residual (honesty).
pub const PARTICLE_REMNANT_WEAPON_NAME: &str = "ParticleUplinkCannonBeamTrailRemnantWeapon";
/// Retail TrailRemnant KindOf residual.
pub const PARTICLE_REMNANT_KIND_OF: &str = "NO_COLLIDE UNATTACKABLE IMMOBILE";
/// Retail TrailRemnant KindOf NO_COLLIDE residual bit honesty.
pub const PARTICLE_REMNANT_KIND_OF_NO_COLLIDE: bool = true;
/// Retail TrailRemnant KindOf UNATTACKABLE residual bit honesty.
pub const PARTICLE_REMNANT_KIND_OF_UNATTACKABLE: bool = true;
/// Retail TrailRemnant KindOf IMMOBILE residual bit honesty.
pub const PARTICLE_REMNANT_KIND_OF_IMMOBILE: bool = true;
/// Retail TrailRemnant ImmortalBody MaxHealth residual.
pub const PARTICLE_REMNANT_MAX_HEALTH: f32 = 50.0;
/// Retail TrailRemnant ImmortalBody InitialHealth residual.
pub const PARTICLE_REMNANT_INITIAL_HEALTH: f32 = 50.0;
/// Retail TrailRemnant EditorSorting residual.
pub const PARTICLE_REMNANT_EDITOR_SORTING: &str = "SYSTEM";
/// Retail TrailRemnant Body module residual.
pub const PARTICLE_REMNANT_BODY: &str = "ImmortalBody";
/// Retail TrailRemnant weapon DamageType residual.
pub const PARTICLE_REMNANT_DAMAGE_TYPE: &str = "PARTICLE_BEAM";
/// Retail TrailRemnant weapon DeathType residual.
pub const PARTICLE_REMNANT_DEATH_TYPE: &str = "BURNED";
/// Retail TrailRemnant FireWeaponUpdate module residual present.
pub const PARTICLE_REMNANT_FIRE_WEAPON_UPDATE: bool = true;
/// Retail TrailRemnant DeletionUpdate module residual present.
pub const PARTICLE_REMNANT_DELETION_UPDATE: bool = true;
/// Retail remnant weapon RadiusDamageAffects residual.
pub const PARTICLE_REMNANT_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS";
/// Retail remnant weapon WeaponSpeed residual (dist/sec).
pub const PARTICLE_REMNANT_WEAPON_SPEED: f32 = 250.0;
/// Retail DeletionUpdate MinLifetime residual frames (4000 ms → 120 @ 30 FPS).
pub const PARTICLE_REMNANT_DELETION_MIN_FRAMES: u32 =
    (PARTICLE_REMNANT_MIN_LIFETIME_MS * 30) / 1000;
/// Retail DeletionUpdate MaxLifetime residual frames (same as min for remnant).
pub const PARTICLE_REMNANT_DELETION_MAX_FRAMES: u32 =
    (PARTICLE_REMNANT_MAX_LIFETIME_MS * 30) / 1000;

/// Host residual for C++ `DeletionUpdate::calcSleepDelay`.
///
/// `delay = GameLogicRandomValue(min, max); if delay < 1 { delay = 1 }`.
/// When min==max (TrailRemnant), delay is deterministic. Fail-closed: not full
/// ThingFactory Object destroy on dieFrame.
#[inline]
pub fn deletion_update_calc_sleep_delay(min_frames: u32, max_frames: u32, random_draw: u32) -> u32 {
    let lo = min_frames.min(max_frames);
    let hi = min_frames.max(max_frames);
    let delay = if lo == hi {
        lo
    } else {
        // residual: clamp random_draw into [lo, hi]
        lo + (random_draw % (hi - lo + 1))
    };
    delay.max(1)
}

/// TrailRemnant fixed DeletionUpdate sleep residual (min==max → 120 frames).
#[inline]
pub fn particle_remnant_deletion_sleep_frames() -> u32 {
    deletion_update_calc_sleep_delay(
        PARTICLE_REMNANT_DELETION_MIN_FRAMES,
        PARTICLE_REMNANT_DELETION_MAX_FRAMES,
        0,
    )
}

/// C++ `ParticleUplinkCannonUpdate` pulse DamageInfo types.
///
/// Prefer leftover `ParticleUplinkCannonUpdateModuleData` (INI-authored).
/// Residual fallback is retail PARTICLE_BEAM / LASERED, not UNRESISTABLE.
pub fn particle_beam_authored_types(
    source_template: Option<&str>,
) -> (
    crate::game_logic::combat::DamageType,
    crate::game_logic::host_usa_pilot::HostDeathType,
) {
    if let Some(name) = source_template {
        if let Some(pair) = leftover_puc_authored_types(name) {
            return pair;
        }
    }
    (
        crate::game_logic::combat::DamageType::ParticleBeam,
        crate::game_logic::host_usa_pilot::HostDeathType::Lasered,
    )
}

/// C++ TrailRemnant FireWeaponUpdate weapon types (PARTICLE_BEAM / BURNED).
pub fn particle_remnant_authored_types() -> (
    crate::game_logic::combat::DamageType,
    crate::game_logic::host_usa_pilot::HostDeathType,
) {
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    if crate::game_logic::thing::ThingTemplate::weapon_from_store(PARTICLE_REMNANT_WEAPON_NAME)
        .is_some()
    {
        let damage = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
            PARTICLE_REMNANT_WEAPON_NAME,
        );
        let death = crate::game_logic::host_armor_residual::resolve_host_death_type(
            Some(PARTICLE_REMNANT_WEAPON_NAME),
            damage,
        );
        return (damage, death);
    }
    (
        crate::game_logic::combat::DamageType::ParticleBeam,
        crate::game_logic::host_usa_pilot::HostDeathType::Burned,
    )
}

fn leftover_puc_authored_types(
    template_name: &str,
) -> Option<(
    crate::game_logic::combat::DamageType,
    crate::game_logic::host_usa_pilot::HostDeathType,
)> {
    use std::str::FromStr;
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("ParticleUplinkCannonUpdate")
        {
            continue;
        }
        let ini_damage = entry
            .data
            .get_ini_field("DamageType")
            .and_then(|raw| gamelogic::damage::DamageType::from_str(raw.trim()).ok())
            .map(crate::game_logic::combat::DamageType::from_store);
        let ini_death = entry
            .data
            .get_ini_field("DeathType")
            .map(|raw| parse_puc_death_type(raw.trim()));
        if let Some(damage) = ini_damage {
            return Some((
                damage,
                ini_death.unwrap_or(crate::game_logic::host_usa_pilot::HostDeathType::Lasered),
            ));
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::ParticleUplinkCannonUpdateModuleData>(
        ) {
            return Some((
                crate::game_logic::combat::DamageType::from_store(data.damage_type),
                crate::game_logic::host_usa_pilot::HostDeathType::from_store(data.death_type),
            ));
        }
    }
    None
}

fn parse_puc_death_type(token: &str) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    match token.to_ascii_uppercase().as_str() {
        "LASERED" => HostDeathType::Lasered,
        "BURNED" => HostDeathType::Burned,
        "EXPLODED" => HostDeathType::Exploded,
        "NORMAL" => HostDeathType::Normal,
        "NONE" => HostDeathType::None,
        "DETONATED" => HostDeathType::Detonated,
        _ => HostDeathType::Lasered,
    }
}
