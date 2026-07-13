//! Host special-power / superweapon strike residual.
//!
//! Residual slice: host `DoSpecialPower` for DaisyCutter / A10 / ScudStorm /
//! ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb /
//! ArtilleryBarrage / CruiseMissile queues a real strike that completes with
//! area damage on host GameLogic objects. NuclearMissile also spawns a residual
//! radiation field (`NukeRadiationFieldWeapon`) that ticks after impact.
//! AnthraxBomb also spawns a residual toxin field
//! (`AnthraxBombPoisonFieldWeapon` / `OCL_PoisonFieldAnthraxBomb`) that ticks
//! after impact. SpectreGunship completes orbit insertion with no one-shot
//! blast, then spawns a residual orbit field (`SpectreHowitzerGun` residual)
//! that periodically damages in `AttackAreaRadius` for `OrbitTime`. ParticleCannon
//! (Particle Uplink) completes charge residual with no one-shot blast, then
//! spawns a residual continuous beam field (`ParticleUplinkCannonUpdate`
//! TotalFiringTime / TotalDamagePulses / DamagePerSecond residual) that pulses
//! damage at the target for the beam dwell. CarpetBomb is a delayed multi-strike
//! line residual (`SUPERWEAPON_CarpetBomb` / `CarpetBombWeapon`): after bomber
//! approach delay, applies explosive damage at multiple epicenters along a line
//! through the target (fail-closed vs full B52 OCL drop path / variance /
//! staggered DropDelay). ArtilleryBarrage is a delayed multi-shell scatter
//! residual (`SUPERWEAPON_ArtilleryBarrage1` / `ArtilleryBarrageDamageWeapon`):
//! after DelayDeliveryMax residual, applies explosive damage at multiple shell
//! epicenters within WeaponErrorRadius (fail-closed vs full ChinaArtilleryCannon
//! OCL DeliverPayload / science tiers 12/24/36 / staggered shell drops).
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
//! stack / gamma upgrade / SpectreGunshipUpdate gattling-strafe + howitzer
//! projectile path / ParticleUplinkCannonUpdate outer-node lasers + swath sine
//! wave + manual driving). CarpetBomb residual is multi-point simultaneous
//! blasts with DropVariance residual scatter (not full AmericaJetB52
//! DeliverPayload / per-bomb DropDelay stagger). ArtilleryBarrage residual is multi-point simultaneous
//! shell blasts (not full ChinaArtilleryCannon transport / random
//! WeaponErrorRadius draw / per-shell DelayDelivery stagger / science upgrade
//! FormationSize matrix). CruiseMissile residual is a single MOAB blast (not
//! full loft projectile / HeightDieUpdate / door animation / MOABFlameWeapon
//! tree-ignite).

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

// --- Particle Uplink continuous beam residual (ParticleUplinkCannonUpdate) ---

/// Retail `ParticleUplinkCannonUpdate` TotalFiringTime = 3500 ms → 105 frames @ 30 FPS.
pub const PARTICLE_BEAM_DURATION_FRAMES: u32 = 105;
/// Retail TotalDamagePulses = 40.
pub const PARTICLE_BEAM_TOTAL_PULSES: u32 = 40;
/// Retail DamagePerSecond = 400.
/// damagePerPulse = (TotalFiringFrames/FPS * DamagePerSecond) / TotalDamagePulses
///                 = (105/30 * 400) / 40 = 35.
pub const PARTICLE_BEAM_DAMAGE_PER_PULSE: f32 = 35.0;
/// Residual pulse interval: TotalFiringTime / TotalDamagePulses → 105/40 ≈ 2.625
/// frames. Host residual uses 3-frame fixed cadence (fail-closed vs fractional
/// nextFactor * orbitalLifetime scheduling in C++).
pub const PARTICLE_BEAM_TICK_INTERVAL_FRAMES: u32 = 3;
/// Residual damage radius at target (fail-closed vs laser radius ×
/// DamageRadiusScalar grow/shrink matrix; retail scalar 3.4 on dynamic beam).
pub const PARTICLE_BEAM_RADIUS: f32 = 50.0;
/// Residual ambient cue while beam is annihilating ground.
pub const PARTICLE_BEAM_AUDIO: &str = "ParticleUplinkCannon_GroundAnnihilationSoundLoop";

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
/// Retail `CarpetBombWeapon` PrimaryDamage.
pub const CARPET_BOMB_DAMAGE: f32 = 300.0;
/// Retail `CarpetBombWeapon` PrimaryDamageRadius.
pub const CARPET_BOMB_RADIUS: f32 = 50.0;
/// Bomber approach residual frames before multi-strike damage applies
/// (fail-closed vs full edge-spawn + transit locomotor).
pub const CARPET_BOMB_IMPACT_DELAY_FRAMES: u32 = 90;

// --- Artillery Barrage scatter multi-shell residual (retail SUPERWEAPON_ArtilleryBarrage1) ---

/// Retail `SUPERWEAPON_ArtilleryBarrage1` FormationSize (Level1 fail-closed;
/// science tiers 2/3 use 24/36 — deferred).
pub const ARTILLERY_BARRAGE_SHELL_COUNT: u32 = 12;
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamage.
pub const ARTILLERY_BARRAGE_DAMAGE: f32 = 105.0;
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamageRadius.
pub const ARTILLERY_BARRAGE_RADIUS: f32 = 50.0;
/// Retail DeliverPayload `WeaponErrorRadius` (shell scatter radius around target).
pub const ARTILLERY_BARRAGE_ERROR_RADIUS: f32 = 100.0;
/// Retail DeliverPayload `DelayDeliveryMax` = 3000 ms → 90 frames @ 30 FPS.
pub const ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES: u32 = 90;
/// Residual ring radius for deterministic shell placement inside error radius
/// (retail random draw within WeaponErrorRadius deferred).
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
            // SCUD launch-to-impact residual.
            HostSuperweaponKind::ScudStorm => 150,
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
            HostSuperweaponKind::ScudStorm => 1500.0,
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
            HostSuperweaponKind::ScudStorm => 200.0,
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
            HostSuperweaponKind::ScudStorm => 80.0,
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

    /// Whether impact should spawn a residual toxin / anthrax field.
    pub fn spawns_toxin_field(self) -> bool {
        matches!(self, HostSuperweaponKind::AnthraxBomb)
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

    /// Whether this kind uses multi-point epicenter damage at impact.
    pub fn is_multi_strike(self) -> bool {
        self.is_line_multi_strike() || self.is_scatter_multi_strike()
    }

    /// Residual multi-point shell/bomb epicenters for multi-strike kinds.
    pub fn multi_strike_points(self, target: Vec3) -> Option<Vec<Vec3>> {
        if self.is_line_multi_strike() {
            Some(carpet_bomb_points(target))
        } else if self.is_scatter_multi_strike() {
            Some(artillery_barrage_points(target))
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

/// Deterministic residual DropVariance scatter for bomb index `i`.
///
/// C++ DeliverPayloadAIUpdate:
/// `pos.x += Random(-var.x, var.x); pos.y += Random(-var.y, var.y);`
/// Host residual: deterministic pseudo-scatter in ±variance (not full RNG stream).
/// C++ X/Y horizontal map to host X/Z; C++ Z maps to host Y (vertical).
pub fn drop_variance_offset(index: u32, var_x: f32, var_y: f32, var_z: f32) -> Vec3 {
    // Golden-ratio phase residual for stable, non-zero scatter across indices.
    let phase = (index as f32 + 1.0) * 0.618_033_988_7;
    let fx = if var_x > 0.0 {
        (phase.fract() * 2.0 - 1.0) * var_x
    } else {
        0.0
    };
    let fy = if var_y > 0.0 {
        ((phase + 0.37).fract() * 2.0 - 1.0) * var_y
    } else {
        0.0
    };
    let fz = if var_z > 0.0 {
        ((phase + 0.73).fract() * 2.0 - 1.0) * var_z
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
/// Fail-closed placement: one shell at the click target + remaining shells on a
/// deterministic ring at `ARTILLERY_BARRAGE_RING_RADIUS` inside
/// `WeaponErrorRadius` (retail random draw deferred). Shell count is Level1
/// FormationSize 12 (science tiers 24/36 deferred).
pub fn artillery_barrage_points(target: Vec3) -> Vec<Vec3> {
    let count = ARTILLERY_BARRAGE_SHELL_COUNT.max(1);
    let mut points = Vec::with_capacity(count as usize);
    // Center shell guarantees a hit at the aimed position.
    points.push(target);
    let ring_count = count.saturating_sub(1);
    if ring_count == 0 {
        return points;
    }
    let ring_r = ARTILLERY_BARRAGE_RING_RADIUS.min(ARTILLERY_BARRAGE_ERROR_RADIUS);
    for i in 0..ring_count {
        let angle = (i as f32) * std::f32::consts::TAU / (ring_count as f32);
        points.push(Vec3::new(
            target.x + ring_r * angle.cos(),
            target.y,
            target.z + ring_r * angle.sin(),
        ));
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
}

/// Damage application plan for a single victim (computed before mutable apply).
#[derive(Debug, Clone, Copy)]
pub struct HostStrikeDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
}

/// Result of resolving one strike at impact time.
#[derive(Debug, Clone)]
pub struct HostStrikeImpactPlan {
    pub strike_id: u32,
    pub kind: HostSuperweaponKind,
    pub target_position: Vec3,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostStrikeDamageHit>,
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

/// Residual toxin / anthrax field spawned by AnthraxBomb impact
/// (`OCL_PoisonFieldAnthraxBomb` / `AnthraxBombPoisonFieldWeapon` residual).
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
    /// Parent AnthraxBomb strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
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
    /// Next absolute frame at which orbit damage ticks apply.
    pub next_tick_frame: u32,
    /// Total residual orbit damage applied across all ticks.
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Objects destroyed by this residual field.
    pub objects_destroyed: u32,
    /// Parent SpectreGunship strike id (0 if spawned without a strike).
    pub parent_strike_id: u32,
}

impl HostSpectreOrbitField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
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
}

impl HostParticleBeamField {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
            || self.pulses_made >= PARTICLE_BEAM_TOTAL_PULSES
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
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
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.radiation_spawned_this_frame.clear();
        self.toxin_spawned_this_frame.clear();
        self.orbit_spawned_this_frame.clear();
        self.beam_spawned_this_frame.clear();
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
    pub fn queue(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let impact_frame = activate_frame.saturating_add(kind.impact_delay_frames());
        let strike = HostSpecialPowerStrike {
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
        };
        self.strikes.insert(id, strike);
        self.activated_this_frame.push(id);
        id
    }

    /// Compute falloff damage for distance from epicenter.
    pub fn damage_at_distance(kind: HostSuperweaponKind, distance: f32) -> f32 {
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
    /// Multi-strike residuals (CarpetBomb line / ArtilleryBarrage scatter): each
    /// living enemy takes the max damage from any shell/bomb epicenter (not a
    /// single circular blast at the click point only).
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
            let bomb_points = strike.kind.multi_strike_points(strike.target_position);
            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == strike.source_object {
                    continue;
                }
                // Fail-closed residual: do not damage friendlies (same team).
                // Retail ArtilleryBarrageDamageWeapon hits ALLIES too — deferred.
                if team == strike.source_team {
                    continue;
                }
                let dmg = if let Some(ref points) = bomb_points {
                    // Multi-strike: best (nearest) shell/bomb epicenter damage.
                    points
                        .iter()
                        .map(|epicenter| {
                            Self::damage_at_distance(
                                strike.kind,
                                horizontal_distance(pos, *epicenter),
                            )
                        })
                        .fold(0.0_f32, f32::max)
                } else {
                    let dist = horizontal_distance(pos, strike.target_position);
                    Self::damage_at_distance(strike.kind, dist)
                };
                if dmg > 0.0 {
                    hits.push(HostStrikeDamageHit {
                        target_id: id,
                        damage: dmg,
                    });
                }
            }
            plans.push(HostStrikeImpactPlan {
                strike_id: strike.id,
                kind: strike.kind,
                target_position: strike.target_position,
                source_object: strike.source_object,
                source_team: strike.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.strike_id);
        plans
    }

    /// Record impact results after GameLogic applied damage.
    ///
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
        let mut spawn_radiation: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        let mut spawn_toxin: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        let mut spawn_orbit: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        let mut spawn_beam: Option<(ObjectId, super::Team, Vec3, u32)> = None;
        if let Some(strike) = self.strikes.get_mut(&strike_id) {
            if strike.phase == HostStrikePhase::Queued {
                strike.phase = HostStrikePhase::Completed;
                strike.total_damage_applied = total_damage;
                strike.objects_hit = objects_hit;
                strike.objects_destroyed = objects_destroyed;
                self.completed_this_frame.push(strike_id);
                if strike.kind.spawns_radiation() {
                    spawn_radiation = Some((
                        strike.source_object,
                        strike.source_team,
                        strike.target_position,
                        strike.impact_frame,
                    ));
                }
                if strike.kind.spawns_toxin_field() {
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
                    ));
                }
                if strike.kind.spawns_beam_field() {
                    spawn_beam = Some((
                        strike.source_object,
                        strike.source_team,
                        strike.target_position,
                        strike.impact_frame,
                    ));
                }
            }
        }
        if let Some((source, team, pos, impact_frame)) = spawn_radiation {
            self.spawn_radiation_field(source, team, pos, impact_frame, strike_id);
        }
        if let Some((source, team, pos, impact_frame)) = spawn_toxin {
            self.spawn_toxin_field(source, team, pos, impact_frame, strike_id);
        }
        if let Some((source, team, pos, impact_frame)) = spawn_orbit {
            self.spawn_orbit_field(source, team, pos, impact_frame, strike_id);
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

    /// Spawn a residual toxin field at `position` (AnthraxBomb impact).
    pub fn spawn_toxin_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_toxin_id;
        self.next_toxin_id = self.next_toxin_id.saturating_add(1).max(1);
        let field = HostToxinField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(ANTHRAX_TOXIN_DURATION_FRAMES),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
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
                if dist <= ANTHRAX_TOXIN_RADIUS {
                    hits.push(HostToxinDamageHit {
                        target_id: id,
                        damage: ANTHRAX_TOXIN_DAMAGE_PER_TICK,
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
                current_frame.saturating_add(ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES);
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
    pub fn spawn_orbit_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_orbit_id;
        self.next_orbit_id = self.next_orbit_id.saturating_add(1).max(1);
        let field = HostSpectreOrbitField {
            id,
            source_object,
            source_team,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(SPECTRE_ORBIT_DURATION_FRAMES),
            // First howitzer residual tick on orbit insertion frame.
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
        };
        self.orbit_fields.push(field);
        self.orbit_spawned_this_frame.push(id);
        self.orbit_fields_spawned_total = self.orbit_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build Spectre orbit damage plans for all fields whose tick frame has arrived.
    ///
    /// Retail `SpectreHowitzerGun` / `SpectreGattlingGun` hit ALLIES ENEMIES
    /// NEUTRALS. Host residual damages living objects in AttackAreaRadius
    /// except the source launcher object and same-team friendlies (host strike
    /// convention for offensive superweapons). Fail-closed vs gattling strafe
    /// pattern / howitzer projectile / random offset / portable structure
    /// contain gunner path.
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
            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                // Fail-closed residual: do not damage friendlies (same team).
                if team == field.source_team {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= SPECTRE_ORBIT_RADIUS {
                    hits.push(HostSpectreOrbitDamageHit {
                        target_id: id,
                        damage: SPECTRE_ORBIT_DAMAGE_PER_TICK,
                        field_id: field.id,
                    });
                }
            }
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

    /// Record Spectre orbit tick results and advance next_tick_frame.
    pub fn record_orbit_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.orbit_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(SPECTRE_ORBIT_TICK_INTERVAL_FRAMES);
            self.orbit_damage_applications_total = self
                .orbit_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired Spectre orbit fields.
    pub fn prune_expired_orbit(&mut self, current_frame: u32) {
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
            expires_frame: spawn_frame.saturating_add(PARTICLE_BEAM_DURATION_FRAMES),
            // First damage pulse on beam-start frame (retail m_nextDamagePulseFrame = now).
            next_tick_frame: spawn_frame,
            pulses_made: 0,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
        };
        self.beam_fields.push(field);
        self.beam_spawned_this_frame.push(id);
        self.beam_fields_spawned_total = self.beam_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build Particle Uplink beam pulse plans for all fields whose tick frame
    /// has arrived.
    ///
    /// Retail damages all alive objects in beam radius (DamageRadiusScalar ×
    /// laser radius). Host residual damages living objects in
    /// [`PARTICLE_BEAM_RADIUS`] except the source launcher and same-team
    /// friendlies (host strike convention). Fail-closed vs swath sine path /
    /// manual beam driving / remnant trail objects / width grow matrix.
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
            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                // Fail-closed residual: do not damage friendlies (same team).
                if team == field.source_team {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= PARTICLE_BEAM_RADIUS {
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
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record Particle Uplink beam pulse results and advance next_tick_frame.
    pub fn record_beam_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.pulses_made = field.pulses_made.saturating_add(1);
            field.next_tick_frame =
                current_frame.saturating_add(PARTICLE_BEAM_TICK_INTERVAL_FRAMES);
            self.beam_damage_applications_total = self
                .beam_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired Particle Uplink beam fields.
    pub fn prune_expired_beam(&mut self, current_frame: u32) {
        self.beam_fields.retain(|f| !f.is_expired(current_frame));
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
    }

    #[test]
    fn particle_cannon_impact_spawns_beam_and_ticks_damage() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let target = Vec3::new(10.0, 0.0, 0.0);
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

        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), Vec3::new(10.0, 0.0, 0.0), Team::GLA, true), // epicenter
            (ObjectId(3), Vec3::new(40.0, 0.0, 0.0), Team::GLA, true), // in radius (dist 30)
            (ObjectId(4), Vec3::new(200.0, 0.0, 0.0), Team::GLA, true), // out of radius
            (ObjectId(5), Vec3::new(10.0, 0.0, 0.0), Team::China, true), // friendly
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

        // First beam pulse on spawn frame.
        let beam_plans = reg.plan_due_beam_ticks(120, &objects);
        assert_eq!(beam_plans.len(), 1);
        assert_eq!(beam_plans[0].hits.len(), 2); // epicenter + mid-radius enemies
        assert!(beam_plans[0].hits.iter().all(|h| {
            (h.damage - PARTICLE_BEAM_DAMAGE_PER_PULSE).abs() < 0.01
                && (h.target_id == ObjectId(2) || h.target_id == ObjectId(3))
        }));
        assert!(!beam_plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(!beam_plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)));

        reg.record_beam_tick_complete(
            beam_plans[0].field_id,
            PARTICLE_BEAM_DAMAGE_PER_PULSE * 2.0,
            2,
            0,
            120,
        );
        assert!(reg.honesty_beam_damage_ok());
        assert_eq!(
            reg.beam_fields()[0].next_tick_frame,
            120 + PARTICLE_BEAM_TICK_INTERVAL_FRAMES
        );
        assert_eq!(reg.beam_fields()[0].pulses_made, 1);

        // Not due again until interval elapses.
        assert!(reg.plan_due_beam_ticks(120 + 1, &objects).is_empty());
        let later = reg.plan_due_beam_ticks(120 + PARTICLE_BEAM_TICK_INTERVAL_FRAMES, &objects);
        assert_eq!(later.len(), 1);
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
        let center = points[7];
        let outer = points[14];
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), center, Team::USA, true), // center bomb (with variance)
            (ObjectId(3), outer, Team::USA, true),  // outer bomb (with variance)
            (ObjectId(4), Vec3::new(0.0, 0.0, 500.0), Team::USA, true), // far off-line
            (ObjectId(5), center, Team::China, true), // friendly
        ];

        // Before impact: no damage plan.
        assert!(reg
            .plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        let plans = reg.plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        // Center + outer-bomb enemies hit; far + friendly excluded.
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
            && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)));

        reg.record_impact_complete(id, CARPET_BOMB_DAMAGE * 2.0, 2, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::CarpetBomb));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::CarpetBomb));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
        assert!(reg.beam_fields().is_empty());
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
        assert!((ARTILLERY_BARRAGE_ERROR_RADIUS - 100.0).abs() < 0.1);
        assert!((ARTILLERY_BARRAGE_RING_RADIUS - 75.0).abs() < 0.1);
        let points = artillery_barrage_points(Vec3::new(100.0, 0.0, 50.0));
        assert_eq!(points.len(), ARTILLERY_BARRAGE_SHELL_COUNT as usize);
        // First shell at target; remaining on ring inside error radius.
        assert!((points[0].x - 100.0).abs() < 0.1);
        assert!((points[0].z - 50.0).abs() < 0.1);
        for p in points.iter().skip(1) {
            let dist = horizontal_distance(*p, Vec3::new(100.0, 0.0, 50.0));
            assert!(
                (dist - ARTILLERY_BARRAGE_RING_RADIUS).abs() < 0.5,
                "ring shell dist={dist}"
            );
            assert!(dist <= ARTILLERY_BARRAGE_ERROR_RADIUS + 0.1);
        }
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

        // Shells: center + ring at RING_RADIUS. Outer shell at +X ring point.
        let outer = Vec3::new(ARTILLERY_BARRAGE_RING_RADIUS, 0.0, 0.0);
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // center shell
            (ObjectId(3), outer, Team::USA, true),                    // ring shell
            (ObjectId(4), Vec3::new(0.0, 0.0, 500.0), Team::USA, true), // far
            (ObjectId(5), Vec3::new(0.0, 0.0, 0.0), Team::China, true), // friendly
        ];

        // Before impact: no damage plan.
        assert!(reg
            .plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        let plans = reg.plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        // Center + ring-shell enemies hit; far + friendly excluded.
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1));
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)
            && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)));

        reg.record_impact_complete(id, ARTILLERY_BARRAGE_DAMAGE * 2.0, 2, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::ArtilleryBarrage));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
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
            (ObjectId(5), Vec3::new(0.0, 0.0, 0.0), Team::USA, true), // friendly
        ];

        // Before impact: no damage plan.
        assert!(reg
            .plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty());

        let plans = reg.plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES, &objects);
        assert_eq!(plans.len(), 1);
        // Epicenter + near enemy hit; far + friendly excluded.
        assert_eq!(plans[0].hits.len(), 2);
        assert!(plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - CRUISE_MISSILE_DAMAGE).abs() < 0.1));
        assert!(plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3) && h.damage > 0.0));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
        assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(5)));

        reg.record_impact_complete(id, CRUISE_MISSILE_DAMAGE * 2.0, 2, 0);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::CruiseMissile));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::CruiseMissile));
        assert!(reg.radiation_fields().is_empty());
        assert!(reg.toxin_fields().is_empty());
        assert!(reg.orbit_fields().is_empty());
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

        // Orbit tick hits enemies in AttackAreaRadius; excludes source + friendlies.
        let orbit_plans = reg.plan_due_orbit_ticks(90, &objects);
        assert_eq!(orbit_plans.len(), 1);
        assert_eq!(orbit_plans[0].hits.len(), 1);
        assert_eq!(orbit_plans[0].hits[0].target_id, ObjectId(2));
        assert!(
            (orbit_plans[0].hits[0].damage - SPECTRE_ORBIT_DAMAGE_PER_TICK).abs() < 0.01
        );

        reg.record_orbit_tick_complete(orbit_plans[0].field_id, 80.0, 1, 0, 90);
        assert!(reg.honesty_orbit_damage_ok());
        assert_eq!(
            reg.orbit_fields()[0].next_tick_frame,
            90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES
        );

        // Second tick after interval.
        let later = reg.plan_due_orbit_ticks(90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES, &objects);
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].hits.len(), 1);
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
        // Blast residual excludes same-team friendlies (host strike convention).
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - 200.0).abs() < 0.1);

        reg.record_impact_complete(id, 200.0, 1, 0);
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
        // Blast residual excludes same-team friendlies (host strike convention).
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - 3500.0).abs() < 0.1);

        reg.record_impact_complete(id, 3500.0, 1, 1);
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
        assert!((plans[0].hits[0].damage - 2000.0).abs() < 0.01);

        reg.record_impact_complete(id, 2000.0, 1, 1);
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
    fn friendly_fire_excluded_from_plan() {
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
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(3));
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
}
