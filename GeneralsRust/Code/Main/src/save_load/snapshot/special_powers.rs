//! Host superweapon / combat-particle / upgrade-research snapshot residual.

use super::xfer_helpers::{xfer_option, xfer_vec_default};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Snapshot of [`HostSpecialPowerStrikeRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialPowerStrikeRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// All strike records (queued / completed / cancelled), sorted by id on capture.
    pub strikes: Vec<HostSpecialPowerStrike>,
    /// Next residual radiation field id (NuclearMissile).
    #[serde(default = "default_next_radiation_id")]
    pub next_radiation_id: u32,
    /// Active residual radiation fields (NuclearMissile impact residual).
    #[serde(default)]
    pub radiation_fields: Vec<crate::game_logic::special_power_strikes::HostRadiationField>,
    /// Lifetime radiation fields spawned (honesty after prune).
    #[serde(default)]
    pub radiation_fields_spawned_total: u32,
    #[serde(default)]
    pub radiation_objects_spawned: u32,
    /// Lifetime radiation damage applications (honesty after prune).
    #[serde(default)]
    pub radiation_damage_applications_total: u32,
    /// Next residual toxin field id (AnthraxBomb).
    #[serde(default = "default_next_toxin_id")]
    pub next_toxin_id: u32,
    /// Active residual toxin fields (AnthraxBomb impact residual).
    #[serde(default)]
    pub toxin_fields: Vec<crate::game_logic::special_power_strikes::HostToxinField>,
    /// Lifetime toxin fields spawned (honesty after prune).
    #[serde(default)]
    pub toxin_fields_spawned_total: u32,
    #[serde(default)]
    pub toxin_objects_spawned: u32,
    /// Lifetime toxin damage applications (honesty after prune).
    #[serde(default)]
    pub toxin_damage_applications_total: u32,
    /// Next residual Spectre orbit field id (SpectreGunship).
    #[serde(default = "default_next_orbit_id")]
    pub next_orbit_id: u32,
    /// Active residual Spectre orbit fields (SpectreGunship residual).
    #[serde(default)]
    pub orbit_fields: Vec<crate::game_logic::special_power_strikes::HostSpectreOrbitField>,
    /// Lifetime orbit fields spawned (honesty after prune).
    #[serde(default)]
    pub orbit_fields_spawned_total: u32,
    /// Lifetime orbit damage applications (honesty after prune).
    #[serde(default)]
    pub orbit_damage_applications_total: u32,
    /// Next residual Particle Uplink beam field id (ParticleCannon).
    #[serde(default = "default_next_beam_id")]
    pub next_beam_id: u32,
    /// Active residual Particle Uplink continuous beam fields.
    #[serde(default)]
    pub beam_fields: Vec<crate::game_logic::special_power_strikes::HostParticleBeamField>,
    /// Lifetime beam fields spawned (honesty after prune).
    #[serde(default)]
    pub beam_fields_spawned_total: u32,
    #[serde(default)]
    pub beam_objects_spawned: u32,
    /// Lifetime beam damage applications (honesty after prune).
    #[serde(default)]
    pub beam_damage_applications_total: u32,
    /// Next residual Particle Uplink remnant field id (DamagePulseRemnant).
    #[serde(default = "default_next_remnant_id")]
    pub next_remnant_id: u32,
    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    #[serde(default)]
    pub remnant_fields: Vec<crate::game_logic::special_power_strikes::HostParticleRemnantField>,
    /// Lifetime remnant fields spawned (honesty after prune).
    #[serde(default)]
    pub remnant_fields_spawned_total: u32,
    /// Honesty: ParticleUplinkCannonTrailRemnant objects spawned.
    #[serde(default)]
    pub remnant_objects_spawned: u32,
    /// Lifetime remnant damage applications (honesty after prune).
    #[serde(default)]
    pub remnant_damage_applications_total: u32,
}

fn default_next_radiation_id() -> u32 {
    1
}

fn default_next_toxin_id() -> u32 {
    1
}

fn default_next_orbit_id() -> u32 {
    1
}

fn default_next_beam_id() -> u32 {
    1
}

fn default_next_remnant_id() -> u32 {
    1
}

impl Default for SpecialPowerStrikeRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 1,
            strikes: Vec::new(),
            next_radiation_id: 1,
            radiation_fields: Vec::new(),
            radiation_fields_spawned_total: 0,
            radiation_objects_spawned: 0,
            radiation_damage_applications_total: 0,
            next_toxin_id: 1,
            toxin_fields: Vec::new(),
            toxin_fields_spawned_total: 0,
            toxin_objects_spawned: 0,
            toxin_damage_applications_total: 0,
            next_orbit_id: 1,
            orbit_fields: Vec::new(),
            orbit_fields_spawned_total: 0,
            orbit_damage_applications_total: 0,
            next_beam_id: 1,
            beam_fields: Vec::new(),
            beam_fields_spawned_total: 0,
            beam_objects_spawned: 0,
            beam_damage_applications_total: 0,
            next_remnant_id: 1,
            remnant_fields: Vec::new(),
            remnant_fields_spawned_total: 0,
            remnant_objects_spawned: 0,
            remnant_damage_applications_total: 0,
        }
    }
}

/// Snapshot of [`CombatParticleRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatParticleRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// Active + inactive host particle system entries (presentation residual).
    pub systems: Vec<CombatParticleSystemEntry>,
}

/// Snapshot of [`HostUpgradeRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpgradeRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// All research records (queued / completed / cancelled), sorted by id on capture.
    pub entries: Vec<HostUpgradeResearch>,
}

impl Default for HostUpgradeRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 0,
            entries: Vec::new(),
        }
    }
}

impl Default for CombatParticleRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 1,
            systems: Vec::new(),
        }
    }
}

impl XferData for HostSuperweaponKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostSuperweaponKind::DaisyCutter => 0u32,
            HostSuperweaponKind::A10Strike => 1,
            HostSuperweaponKind::ScudStorm => 2,
            HostSuperweaponKind::ParticleCannon => 3,
            HostSuperweaponKind::NuclearMissile => 4,
            HostSuperweaponKind::AnthraxBomb => 5,
            HostSuperweaponKind::SpectreGunship => 6,
            HostSuperweaponKind::CarpetBomb => 7,
            HostSuperweaponKind::ArtilleryBarrage => 8,
            HostSuperweaponKind::CruiseMissile => 9,
            HostSuperweaponKind::NapalmStrike => 10,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostSuperweaponKind::DaisyCutter,
            1 => HostSuperweaponKind::A10Strike,
            2 => HostSuperweaponKind::ScudStorm,
            3 => HostSuperweaponKind::ParticleCannon,
            4 => HostSuperweaponKind::NuclearMissile,
            5 => HostSuperweaponKind::AnthraxBomb,
            6 => HostSuperweaponKind::SpectreGunship,
            7 => HostSuperweaponKind::CarpetBomb,
            8 => HostSuperweaponKind::ArtilleryBarrage,
            9 => HostSuperweaponKind::CruiseMissile,
            10 => HostSuperweaponKind::NapalmStrike,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostSuperweaponKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostStrikePhase {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostStrikePhase::Queued => 0u32,
            HostStrikePhase::Completed => 1,
            HostStrikePhase::Cancelled => 2,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostStrikePhase::Queued,
            1 => HostStrikePhase::Completed,
            2 => HostStrikePhase::Cancelled,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostStrikePhase discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostSpecialPowerStrike {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostSpecialPowerStrike")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("TargetPosition")?;
        self.target_position.xfer(xfer)?;
        xfer.xfer_marker_label("ActivateFrame")?;
        xfer.xfer_u32(&mut self.activate_frame)?;
        xfer.xfer_marker_label("ImpactFrame")?;
        xfer.xfer_u32(&mut self.impact_frame)?;
        xfer.xfer_marker_label("Phase")?;
        self.phase.xfer(xfer)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("ObjectsHit")?;
        xfer.xfer_u32(&mut self.objects_hit)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        // Appended: skip registry 300/50 when leftover flight owns CarpetBombWeapon.
        xfer.xfer_marker_label("LiveCarpetDelivery")?;
        xfer.xfer_bool(&mut self.live_carpet_delivery)?;
        xfer.xfer_marker_label("ManualBeamHold")?;
        xfer.xfer_bool(&mut self.manual_beam_hold)?;
        // Appended: skip registry 200/100 + toxin when leftover flight owns FireWeaponWhenDead.
        xfer.xfer_marker_label("LiveAnthraxDelivery")?;
        xfer.xfer_bool(&mut self.live_anthrax_delivery)?;
        xfer.xfer_marker_label("ScriptedWaypointMode")?;
        xfer.xfer_bool(&mut self.scripted_waypoint_mode)?;
        xfer.xfer_marker_label("NextDestWaypointId")?;
        xfer.xfer_u32(&mut self.next_dest_waypoint_id)?;
        xfer.xfer_marker_label("WaypointOverride")?;
        self.waypoint_override.xfer(xfer)?;
        // Appended: skip registry impact wave when leftover flight owns A10 missiles.
        xfer.xfer_marker_label("LiveA10Delivery")?;
        xfer.xfer_bool(&mut self.live_a10_delivery)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostRadiationField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostRadiationField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("ObjectId")?;
        xfer_option(xfer, &mut self.object_id, ObjectId(0))?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // Wave 56: radiation residual pack honesty counters (appended).
        xfer.xfer_marker_label("RadiationResidualPackArmed")?;
        xfer.xfer_u32(&mut self.radiation_residual_pack_armed)?;
        xfer.xfer_marker_label("RadiationSuspendFxApplications")?;
        xfer.xfer_u32(&mut self.radiation_suspend_fx_applications)?;
        xfer.xfer_marker_label("RadiationFireFxApplications")?;
        xfer.xfer_u32(&mut self.radiation_fire_fx_applications)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostToxinField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostToxinField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("ObjectId")?;
        xfer_option(xfer, &mut self.object_id, ObjectId(0))?;
        xfer.xfer_marker_label("ObjectTemplate")?;
        xfer.xfer_string(&mut self.object_template)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // LargePoisonField / Anthrax residual params (appended after parent id).
        xfer.xfer_marker_label("DamagePerTick")?;
        xfer.xfer_f32(&mut self.damage_per_tick)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        xfer.xfer_marker_label("TickIntervalFrames")?;
        xfer.xfer_u32(&mut self.tick_interval_frames)?;
        // Wave 56: toxin residual pack honesty counters (appended).
        xfer.xfer_marker_label("ToxinResidualPackArmed")?;
        xfer.xfer_u32(&mut self.toxin_residual_pack_armed)?;
        xfer.xfer_marker_label("ToxinFireFxApplications")?;
        xfer.xfer_u32(&mut self.toxin_fire_fx_applications)?;
        xfer.xfer_marker_label("ToxinDamageTypeApplications")?;
        xfer.xfer_u32(&mut self.toxin_damage_type_applications)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostSpectreOrbitField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostSpectreOrbitField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // Gattling/howitzer residual bookkeeping (appended).
        xfer.xfer_marker_label("NextGattlingTickFrame")?;
        xfer.xfer_u32(&mut self.next_gattling_tick_frame)?;
        xfer.xfer_marker_label("HowitzerTicks")?;
        xfer.xfer_u32(&mut self.howitzer_ticks)?;
        xfer.xfer_marker_label("GattlingTicks")?;
        xfer.xfer_u32(&mut self.gattling_ticks)?;
        // Continuous-fire residual bookkeeping (appended).
        xfer.xfer_marker_label("GattlingConsecutive")?;
        xfer.xfer_u32(&mut self.gattling_consecutive)?;
        xfer.xfer_marker_label("HowitzerConsecutive")?;
        xfer.xfer_u32(&mut self.howitzer_consecutive)?;
        xfer.xfer_marker_label("GattlingFireLevel")?;
        xfer.xfer_u8(&mut self.gattling_fire_level)?;
        xfer.xfer_marker_label("HowitzerFireLevel")?;
        xfer.xfer_u8(&mut self.howitzer_fire_level)?;
        // ContinuousFireCoast residual bookkeeping (appended).
        xfer.xfer_marker_label("GattlingCoastUntilFrame")?;
        xfer.xfer_u32(&mut self.gattling_coast_until_frame)?;
        xfer.xfer_marker_label("HowitzerCoastUntilFrame")?;
        xfer.xfer_u32(&mut self.howitzer_coast_until_frame)?;
        xfer.xfer_marker_label("GattlingCoastApplications")?;
        xfer.xfer_u32(&mut self.gattling_coast_applications)?;
        xfer.xfer_marker_label("HowitzerCoastApplications")?;
        xfer.xfer_u32(&mut self.howitzer_coast_applications)?;
        xfer.xfer_marker_label("RapidFireVoiceCues")?;
        xfer.xfer_u32(&mut self.rapid_fire_voice_cues)?;
        // MODELCONDITION_CONTINUOUS_FIRE_* residual bookkeeping (appended).
        xfer.xfer_marker_label("ModelConditionMeanSets")?;
        xfer.xfer_u32(&mut self.model_condition_mean_sets)?;
        xfer.xfer_marker_label("ModelConditionFastSets")?;
        xfer.xfer_u32(&mut self.model_condition_fast_sets)?;
        xfer.xfer_marker_label("ModelConditionSlowSets")?;
        xfer.xfer_u32(&mut self.model_condition_slow_sets)?;
        // SpectreHowitzerShell projectile residual (appended).
        xfer.xfer_marker_label("HowitzerShellsSpawned")?;
        xfer.xfer_u32(&mut self.howitzer_shells_spawned)?;
        xfer.xfer_marker_label("HowitzerShellFireFx")?;
        xfer.xfer_u32(&mut self.howitzer_shell_fire_fx)?;
        xfer.xfer_marker_label("HowitzerShellDetonationFx")?;
        xfer.xfer_u32(&mut self.howitzer_shell_detonation_fx)?;
        xfer.xfer_marker_label("HowitzerShellHeightDieDelays")?;
        xfer.xfer_u32(&mut self.howitzer_shell_height_die_delays)?;
        xfer.xfer_marker_label("HowitzerShellFireSounds")?;
        xfer.xfer_u32(&mut self.howitzer_shell_fire_sounds)?;
        // SpectreHowitzerShell DumbProjectile / Physics / InstantDeath residual.
        xfer.xfer_marker_label("HowitzerShellDumbProjectileApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_dumb_projectile_applications)?;
        xfer.xfer_marker_label("HowitzerShellPhysicsMassApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_physics_mass_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathDetonatedApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_detonated_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathLaseredApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_lasered_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathLaseredOclApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_lasered_ocl_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathGenericApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_generic_applications)?;
        xfer.xfer_marker_label("HowitzerShellObjectParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_object_params_applications)?;
        xfer.xfer_marker_label("HowitzerShellDesignParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_design_params_applications)?;
        xfer.xfer_marker_label("HowitzerShellOnlyMovingDownApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_only_moving_down_applications)?;
        // SpectreHowitzerShell W3D ModelDraw residual (appended).
        xfer.xfer_marker_label("HowitzerShellModelDrawApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_model_draw_applications)?;
        xfer.xfer_marker_label("HowitzerShellScaleApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_scale_applications)?;
        xfer.xfer_marker_label("HowitzerShellShadowApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_shadow_applications)?;
        xfer.xfer_marker_label("HowitzerShellGeometryApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_geometry_applications)?;
        xfer.xfer_marker_label("HowitzerShellMaxHealthApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_max_health_applications)?;
        // SpectreHowitzerShell loft flight residual (appended).
        xfer.xfer_marker_label("HowitzerShellLoftFlightApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_loft_flight_applications)?;
        xfer.xfer_marker_label("HowitzerShellLastLoftHeight")?;
        xfer.xfer_f32(&mut self.howitzer_shell_last_loft_height)?;
        xfer.xfer_marker_label("HowitzerShellLoftHeightDieApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_loft_height_die_applications)?;
        // SpectreHowitzerShellLocomotor template + Armor DamageFX residual (appended).
        xfer.xfer_marker_label("HowitzerShellLocomotorTemplateApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_locomotor_template_applications)?;
        xfer.xfer_marker_label("HowitzerShellDamageFxApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_damage_fx_applications)?;
        // Wave 74: SpectreHowitzerShell ThingFactory spawn bookkeeping residual.
        xfer.xfer_marker_label("HowitzerShellThingFactorySpawnApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_thing_factory_spawn_applications)?;
        xfer.xfer_marker_label("HowitzerGunAimParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_aim_params_applications)?;
        xfer.xfer_marker_label("HowitzerGunFireParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_fire_params_applications)?;
        // SpectreHowitzerGun anti residual (appended).
        xfer.xfer_marker_label("HowitzerGunAntiParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_anti_params_applications)?;
        // SpectreGattlingGun anti/fire residual (appended).
        xfer.xfer_marker_label("GattlingGunParamsApplications")?;
        xfer.xfer_u32(&mut self.gattling_gun_params_applications)?;
        // Wave 50: ContinuousFire WeaponBonus ROF residual applications (appended).
        xfer.xfer_marker_label("GattlingRofMeanApplications")?;
        xfer.xfer_u32(&mut self.gattling_rof_mean_applications)?;
        xfer.xfer_marker_label("GattlingRofFastApplications")?;
        xfer.xfer_u32(&mut self.gattling_rof_fast_applications)?;
        // C++ m_overrideTargetDestination (clamped reticle; epicenter stays).
        xfer.xfer_marker_label("OverrideDestination")?;
        self.override_destination.xfer(xfer)?;
        // C++ m_gattlingTargetPosition / m_positionToShootAt / FollowLag counter.
        xfer.xfer_marker_label("GattlingTargetPosition")?;
        self.gattling_target_position.xfer(xfer)?;
        xfer.xfer_marker_label("PositionToShootAt")?;
        self.position_to_shoot_at.xfer(xfer)?;
        xfer.xfer_marker_label("OkToFireHowitzerCounter")?;
        xfer.xfer_u32(&mut self.ok_to_fire_howitzer_counter)?;

        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostParticleBeamField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostParticleBeamField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("ObjectId")?;
        xfer_option(xfer, &mut self.object_id, ObjectId(0))?;
        xfer.xfer_marker_label("ConnectorObjectIds")?;
        xfer_vec_default(xfer, &mut self.connector_object_ids, ObjectId(0))?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("PulsesMade")?;
        xfer.xfer_u32(&mut self.pulses_made)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // SwathOfDeath residual bookkeeping (appended).
        xfer.xfer_marker_label("LastSwathPosition")?;
        self.last_swath_position.xfer(xfer)?;
        xfer.xfer_marker_label("MaxSwathOffset")?;
        xfer.xfer_f32(&mut self.max_swath_offset)?;
        xfer.xfer_marker_label("SwathApplications")?;
        xfer.xfer_u32(&mut self.swath_applications)?;
        // WidthGrow + TotalScorchMarks / RevealRange residual (appended).
        xfer.xfer_marker_label("NextScorchFrame")?;
        xfer.xfer_u32(&mut self.next_scorch_frame)?;
        xfer.xfer_marker_label("ScorchMarksMade")?;
        xfer.xfer_u32(&mut self.scorch_marks_made)?;
        xfer.xfer_marker_label("RevealApplications")?;
        xfer.xfer_u32(&mut self.reveal_applications)?;
        xfer.xfer_marker_label("GroundHitFxApplications")?;
        xfer.xfer_u32(&mut self.ground_hit_fx_applications)?;
        xfer.xfer_marker_label("PeakWidthScalar")?;
        xfer.xfer_f32(&mut self.peak_width_scalar)?;
        xfer.xfer_marker_label("LastDamageRadius")?;
        xfer.xfer_f32(&mut self.last_damage_radius)?;
        // WidthGrow decay residual honesty (appended after grow fields).
        xfer.xfer_marker_label("LastWidthScalar")?;
        xfer.xfer_f32(&mut self.last_width_scalar)?;
        xfer.xfer_marker_label("TroughWidthScalar")?;
        xfer.xfer_f32(&mut self.trough_width_scalar)?;
        xfer.xfer_marker_label("DecaySamples")?;
        xfer.xfer_u32(&mut self.decay_samples)?;
        xfer.xfer_marker_label("LastScorchPosition")?;
        self.last_scorch_position.xfer(xfer)?;
        xfer.xfer_marker_label("LastScorchRadius")?;
        xfer.xfer_f32(&mut self.last_scorch_radius)?;
        // Manual beam driving + outer-node/connector laser residual (appended).
        xfer.xfer_marker_label("ManualTargetMode")?;
        xfer.xfer_bool(&mut self.manual_target_mode)?;
        xfer.xfer_marker_label("OverrideDestination")?;
        self.override_destination.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentTargetPosition")?;
        self.current_target_position.xfer(xfer)?;
        xfer.xfer_marker_label("LastDrivingClickFrame")?;
        xfer.xfer_u32(&mut self.last_driving_click_frame)?;
        xfer.xfer_marker_label("SecondLastDrivingClickFrame")?;
        xfer.xfer_u32(&mut self.second_last_driving_click_frame)?;
        xfer.xfer_marker_label("LastDriveUpdateFrame")?;
        xfer.xfer_u32(&mut self.last_drive_update_frame)?;
        xfer.xfer_marker_label("ManualDriveDistanceTotal")?;
        xfer.xfer_f32(&mut self.manual_drive_distance_total)?;
        xfer.xfer_marker_label("ManualDriveApplications")?;
        xfer.xfer_u32(&mut self.manual_drive_applications)?;
        xfer.xfer_marker_label("FastDriveApplications")?;
        xfer.xfer_u32(&mut self.fast_drive_applications)?;
        xfer.xfer_marker_label("ScriptedWaypointMode")?;
        xfer.xfer_bool(&mut self.scripted_waypoint_mode)?;
        xfer.xfer_marker_label("NextDestWaypointId")?;
        xfer.xfer_u32(&mut self.next_dest_waypoint_id)?;
        xfer.xfer_marker_label("OuterNodeSystemsCreated")?;
        xfer.xfer_u32(&mut self.outer_node_systems_created)?;
        xfer.xfer_marker_label("ConnectorLasersCreated")?;
        xfer.xfer_u32(&mut self.connector_lasers_created)?;
        xfer.xfer_marker_label("LaserBaseFlareCreated")?;
        xfer.xfer_u32(&mut self.laser_base_flare_created)?;
        xfer.xfer_marker_label("GroundToOrbitLaserCreated")?;
        xfer.xfer_u32(&mut self.ground_to_orbit_laser_created)?;
        // Intensity schedule residual (CHARGING…POSTFIRE/PACKING + BeamLaunchFX).
        xfer.xfer_marker_label("Status")?;
        {
            let mut v = self.status.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.status =
                crate::game_logic::special_power_strikes::ParticleUplinkStatus::from_u8(v);
        }
        xfer.xfer_marker_label("OuterIntensity")?;
        {
            let mut v = self.outer_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.outer_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("ConnectorIntensity")?;
        {
            let mut v = self.connector_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.connector_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("LaserBaseIntensity")?;
        {
            let mut v = self.laser_base_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.laser_base_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("BeamLaunchFxApplications")?;
        xfer.xfer_u32(&mut self.beam_launch_fx_applications)?;
        xfer.xfer_marker_label("NextLaunchFxFrame")?;
        xfer.xfer_u32(&mut self.next_launch_fx_frame)?;
        xfer.xfer_marker_label("PostfireApplications")?;
        xfer.xfer_u32(&mut self.postfire_applications)?;
        xfer.xfer_marker_label("PackingApplications")?;
        xfer.xfer_u32(&mut self.packing_applications)?;
        xfer.xfer_marker_label("IntensityTransitions")?;
        xfer.xfer_u32(&mut self.intensity_transitions)?;
        xfer.xfer_marker_label("ConnectorFlareCreated")?;
        xfer.xfer_u32(&mut self.connector_flare_created)?;
        // OuterBeamWidth × scalar / retail laser radius residual (appended).
        xfer.xfer_marker_label("PeakOuterBeamDrawWidth")?;
        xfer.xfer_f32(&mut self.peak_outer_beam_draw_width)?;
        xfer.xfer_marker_label("LastOuterBeamDrawWidth")?;
        xfer.xfer_f32(&mut self.last_outer_beam_draw_width)?;
        xfer.xfer_marker_label("PeakRetailLaserRadius")?;
        xfer.xfer_f32(&mut self.peak_retail_laser_radius)?;
        xfer.xfer_marker_label("LastRetailLaserRadius")?;
        xfer.xfer_f32(&mut self.last_retail_laser_radius)?;
        xfer.xfer_marker_label("PeakRetailDamageRadius")?;
        xfer.xfer_f32(&mut self.peak_retail_damage_radius)?;
        xfer.xfer_marker_label("LastRetailDamageRadius")?;
        xfer.xfer_f32(&mut self.last_retail_damage_radius)?;
        xfer.xfer_marker_label("OrbitalLaserDrawParamsArmed")?;
        xfer.xfer_u32(&mut self.orbital_laser_draw_params_armed)?;
        xfer.xfer_marker_label("ConnectorOuterBeamWidthArmed")?;
        xfer.xfer_u32(&mut self.connector_outer_beam_width_armed)?;
        // Multi-beam NumBeams + ScrollRate residual (appended).
        xfer.xfer_marker_label("NumBeamsArmed")?;
        xfer.xfer_u32(&mut self.num_beams_armed)?;
        xfer.xfer_marker_label("TilingScalarArmed")?;
        xfer.xfer_u32(&mut self.tiling_scalar_armed)?;
        xfer.xfer_marker_label("LastScrollUv")?;
        xfer.xfer_f32(&mut self.last_scroll_uv)?;
        xfer.xfer_marker_label("PeakAbsScrollUv")?;
        xfer.xfer_f32(&mut self.peak_abs_scroll_uv)?;
        xfer.xfer_marker_label("ScrollUvSamples")?;
        xfer.xfer_u32(&mut self.scroll_uv_samples)?;
        // Multi-beam soft-edge residual (appended).
        xfer.xfer_marker_label("SoftEdgeSamples")?;
        xfer.xfer_u32(&mut self.soft_edge_samples)?;
        xfer.xfer_marker_label("PeakSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_soft_edge_outer_width)?;
        xfer.xfer_marker_label("LastSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.last_soft_edge_outer_width)?;
        xfer.xfer_marker_label("LastSoftEdgeOuterAlpha")?;
        xfer.xfer_f32(&mut self.last_soft_edge_outer_alpha)?;
        xfer.xfer_marker_label("LastSoftEdgeTileFactor")?;
        xfer.xfer_f32(&mut self.last_soft_edge_tile_factor)?;
        xfer.xfer_marker_label("SoftEdgeColorArmed")?;
        xfer.xfer_u32(&mut self.soft_edge_color_armed)?;
        xfer.xfer_marker_label("SoftEdgePremulSamples")?;
        xfer.xfer_u32(&mut self.soft_edge_premul_samples)?;
        xfer.xfer_marker_label("LastSoftEdgePremulOuterR")?;
        xfer.xfer_f32(&mut self.last_soft_edge_premul_outer_r)?;
        // Connector soft-edge premul + Orbital KindOf/Segments residual (appended).
        xfer.xfer_marker_label("ConnectorSoftEdgePremulSamples")?;
        xfer.xfer_u32(&mut self.connector_soft_edge_premul_samples)?;
        xfer.xfer_marker_label("LastConnectorSoftEdgePremulOuterR")?;
        xfer.xfer_f32(&mut self.last_connector_soft_edge_premul_outer_r)?;
        xfer.xfer_marker_label("OrbitalKindofImmobileArmed")?;
        xfer.xfer_u32(&mut self.orbital_kindof_immobile_armed)?;
        xfer.xfer_marker_label("OrbitalSegmentsArmed")?;
        xfer.xfer_u32(&mut self.orbital_segments_armed)?;
        xfer.xfer_marker_label("OrbitalArcHeightArmed")?;
        xfer.xfer_u32(&mut self.orbital_arc_height_armed)?;
        // Connector KindOf / Segments / MaxIntensity / Tile residual (appended).
        xfer.xfer_marker_label("ConnectorKindofImmobileArmed")?;
        xfer.xfer_u32(&mut self.connector_kindof_immobile_armed)?;
        xfer.xfer_marker_label("ConnectorSegmentsArmed")?;
        xfer.xfer_u32(&mut self.connector_segments_armed)?;
        xfer.xfer_marker_label("ConnectorMaxIntensityFadeArmed")?;
        xfer.xfer_u32(&mut self.connector_max_intensity_fade_armed)?;
        xfer.xfer_marker_label("ConnectorTileNoArmed")?;
        xfer.xfer_u32(&mut self.connector_tile_no_armed)?;
        // Outer-node bone layout residual (appended).
        xfer.xfer_marker_label("OuterNodeBoneLayoutApplications")?;
        xfer.xfer_u32(&mut self.outer_node_bone_layout_applications)?;
        xfer.xfer_marker_label("LastOuterNodeBonePosition")?;
        self.last_outer_node_bone_position.xfer(xfer)?;
        xfer.xfer_marker_label("ConnectorBoneLayoutApplications")?;
        xfer.xfer_u32(&mut self.connector_bone_layout_applications)?;
        // Intense connector soft-edge + laser segments residual (appended).
        xfer.xfer_marker_label("ConnectorSoftEdgeArmed")?;
        xfer.xfer_u32(&mut self.connector_soft_edge_armed)?;
        xfer.xfer_marker_label("PeakConnectorSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_connector_soft_edge_outer_width)?;
        xfer.xfer_marker_label("ConnectorLaserSegmentsCreated")?;
        xfer.xfer_u32(&mut self.connector_laser_segments_created)?;
        xfer.xfer_marker_label("LastConnectorSegmentStart")?;
        self.last_connector_segment_start.xfer(xfer)?;
        xfer.xfer_marker_label("LastConnectorSegmentEnd")?;
        self.last_connector_segment_end.xfer(xfer)?;
        // Medium connector soft-edge + OrbitalLaser Vision/Shroud residual (appended).
        xfer.xfer_marker_label("MediumConnectorSoftEdgeArmed")?;
        xfer.xfer_u32(&mut self.medium_connector_soft_edge_armed)?;
        xfer.xfer_marker_label("PeakMediumConnectorSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_medium_connector_soft_edge_outer_width)?;
        xfer.xfer_marker_label("OrbitalVisionShroudArmed")?;
        xfer.xfer_u32(&mut self.orbital_vision_shroud_armed)?;
        xfer.xfer_marker_label("LastOrbitalVisionRange")?;
        xfer.xfer_f32(&mut self.last_orbital_vision_range)?;
        xfer.xfer_marker_label("LastOrbitalShroudClearingRange")?;
        xfer.xfer_f32(&mut self.last_orbital_shroud_clearing_range)?;
        // LaserUpdate client residual (appended).
        xfer.xfer_marker_label("LaserUpdateInitApplications")?;
        xfer.xfer_u32(&mut self.laser_update_init_applications)?;
        xfer.xfer_marker_label("LaserUpdateDirty")?;
        xfer.xfer_bool(&mut self.laser_update_dirty)?;
        xfer.xfer_marker_label("LaserUpdateGrowthFrames")?;
        xfer.xfer_u32(&mut self.laser_update_growth_frames)?;
        xfer.xfer_marker_label("LaserUpdateCurrentWidthScalar")?;
        xfer.xfer_f32(&mut self.laser_update_current_width_scalar)?;
        xfer.xfer_marker_label("LaserUpdateWidening")?;
        xfer.xfer_bool(&mut self.laser_update_widening)?;
        xfer.xfer_marker_label("LaserUpdateDecaying")?;
        xfer.xfer_bool(&mut self.laser_update_decaying)?;
        xfer.xfer_marker_label("LastLaserUpdateStart")?;
        self.last_laser_update_start.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateEnd")?;
        self.last_laser_update_end.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateDrawableMid")?;
        self.last_laser_update_drawable_mid.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateRadius")?;
        xfer.xfer_f32(&mut self.last_laser_update_radius)?;
        // Wave 45: PUC sound / scorch residual pack honesty (appended).
        xfer.xfer_marker_label("GroundAnnihilationAudioApplications")?;
        xfer.xfer_u32(&mut self.ground_annihilation_audio_applications)?;
        xfer.xfer_marker_label("FiringToPackAudioApplications")?;
        xfer.xfer_u32(&mut self.firing_to_pack_audio_applications)?;
        xfer.xfer_marker_label("SoundResidualPackArmed")?;
        xfer.xfer_u32(&mut self.sound_residual_pack_armed)?;
        xfer.xfer_marker_label("ScorchScalarPackArmed")?;
        xfer.xfer_u32(&mut self.scorch_scalar_pack_armed)?;
        // Wave 50: OuterNodes flare pack + SlowDeath/InstantDeath pack (appended).
        xfer.xfer_marker_label("OuterNodeFlarePackArmed")?;
        xfer.xfer_u32(&mut self.outer_node_flare_pack_armed)?;
        xfer.xfer_marker_label("DeathPackArmed")?;
        xfer.xfer_u32(&mut self.death_pack_armed)?;
        xfer.xfer_marker_label("StartDecayFrame")?;
        xfer.xfer_u32(&mut self.start_decay_frame)?;
        // SwathOfDeath building→target axis (appended).
        xfer.xfer_marker_label("SourcePosition")?;
        self.source_position.xfer(xfer)?;
        xfer.xfer_marker_label("SourceAxisSet")?;
        xfer.xfer_bool(&mut self.source_axis_set)?;
        Ok(())
    }
}

impl XferData for SpecialPowerStrikeRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("SpecialPowerStrikeRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Strikes")?;
        xfer_vec_default(
            xfer,
            &mut self.strikes,
            HostSpecialPowerStrike {
                id: 0,
                kind: HostSuperweaponKind::DaisyCutter,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                target_position: Vec3::ZERO,
                activate_frame: 0,
                impact_frame: 0,
                phase: HostStrikePhase::Queued,
                total_damage_applied: 0.0,
                objects_hit: 0,
                objects_destroyed: 0,
                artillery_tier:
                    crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier::Level1,
                spectre_tier:
                    crate::game_logic::special_power_strikes::SpectreGunshipScienceTier::Level2,
                scud_anthrax_tier:
                    crate::game_logic::special_power_strikes::ScudStormAnthraxTier::Base,
                a10_tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier::Level1,
                a10_formation_size_applications: 0,
                multi_strike_applied: 0,
                particle_status:
                    crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                particle_status_peak:
                    crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                particle_intensity_transitions: 0,
                particle_charging_applications: 0,
                particle_preparing_applications: 0,
                particle_almost_ready_applications: 0,
                particle_ready_applications: 0,
                particle_model_unpacking_sets: 0,
                particle_model_deployed_sets: 0,
                particle_model_packing_sets: 0,
                particle_powerup_audio_applications: 0,
                particle_unpack_audio_applications: 0,
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
                scud_loft_phase_peak:
                    crate::game_logic::special_power_strikes::ScudMissileLoftPhase::Loft,
                scud_last_spring_height: 0.0,
                scud_ballistic_flight_applications: 0,
                scud_only_moving_down_applications: 0,
                scud_snap_to_ground_applications: 0,
                scud_model_draw_applications: 0,
                scud_last_flight_distance: 0.0,
                scud_peak_flight_distance: 0.0,
                scud_last_flight_height: 0.0,
                scud_thrust_wobble_applications: 0,
                scud_last_thrust_wobble: 0.0,
                scud_peak_abs_thrust_wobble: 0.0,
                scud_geometry_applications: 0,
                scud_object_params_applications: 0,
                scud_missile_ai_applications: 0,
                scud_fire_weapon_when_dead_applications: 0,
                scud_body_draw_params_applications: 0,
                scud_locomotor_appearance_applications: 0,
                scud_destroy_die_locomotor_name_applications: 0,
                scud_death_fire_ocl_applications: 0,
                scud_locomotor_speed_table_applications: 0,
                scud_death_damage_table_applications: 0,
                scud_weapon_launch_applications: 0,
                scud_weapon_special_applications: 0,
                scud_missile_ai_defaults_applications: 0,
                scud_thing_factory_spawn_applications: 0,
                carpet_tier:
                    crate::game_logic::special_power_strikes::CarpetBombFactionTier::America,
                carpet_residual_pack_armed: 0,
                carpet_preferred_height_applications: 0,
                carpet_drop_delay_applications: 0,
                carpet_drop_variance_applications: 0,
                carpet_bomb_count_applications: 0,
                carpet_fire_fx_applications: 0,
                carpet_delivery_distance_applications: 0,
                artillery_residual_pack_armed: 0,
                artillery_cannon_transport_applications: 0,
                artillery_formation_size_applications: 0,
                artillery_delay_delivery_applications: 0,
                artillery_weapon_error_radius_applications: 0,
                artillery_preferred_height_applications: 0,
                artillery_fire_fx_applications: 0,
                cruise_residual_pack_armed: 0,
                cruise_loft_applications: 0,
                cruise_height_die_applications: 0,
                cruise_projectile_applications: 0,
                cruise_moab_weapon_applications: 0,
                cruise_moab_flame_applications: 0,
                cruise_moab_fire_fx_applications: 0,
                nuke_radiation_residual_pack_applications: 0,
                anthrax_toxin_residual_pack_applications: 0,
                live_neutron_delivery: false,
                live_scud_delivery: false,
                live_carpet_delivery: false,
                live_anthrax_delivery: false,
                live_a10_delivery: false,
                manual_beam_hold: false,
                scripted_waypoint_mode: false,
                next_dest_waypoint_id: 0,
                waypoint_override: Vec3::ZERO,
            },
        )?;
        // NuclearMissile residual radiation fields (appended; older binary
        // residual saves without these fields fail-closed on xfer).
        xfer.xfer_marker_label("NextRadiationId")?;
        xfer.xfer_u32(&mut self.next_radiation_id)?;
        xfer.xfer_marker_label("RadiationFields")?;
        xfer_vec_default(
            xfer,
            &mut self.radiation_fields,
            crate::game_logic::special_power_strikes::HostRadiationField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                object_id: None,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                radiation_residual_pack_armed: 0,
                radiation_suspend_fx_applications: 0,
                radiation_fire_fx_applications: 0,
            },
        )?;
        xfer.xfer_marker_label("RadiationFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.radiation_fields_spawned_total)?;
        xfer.xfer_marker_label("RadiationObjectsSpawned")?;
        xfer.xfer_u32(&mut self.radiation_objects_spawned)?;
        xfer.xfer_marker_label("RadiationDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.radiation_damage_applications_total)?;
        // AnthraxBomb residual toxin fields (appended after radiation).
        xfer.xfer_marker_label("NextToxinId")?;
        xfer.xfer_u32(&mut self.next_toxin_id)?;
        xfer.xfer_marker_label("ToxinFields")?;
        xfer_vec_default(
            xfer,
            &mut self.toxin_fields,
            crate::game_logic::special_power_strikes::HostToxinField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                object_id: None,
                object_template: String::new(),
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                toxin_residual_pack_armed: 0,
                toxin_fire_fx_applications: 0,
                toxin_damage_type_applications: 0,
                damage_per_tick:
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DAMAGE_PER_TICK,
                radius: crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_RADIUS,
                tick_interval_frames:
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
            },
        )?;
        xfer.xfer_marker_label("ToxinFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.toxin_fields_spawned_total)?;
        xfer.xfer_marker_label("ToxinObjectsSpawned")?;
        xfer.xfer_u32(&mut self.toxin_objects_spawned)?;
        xfer.xfer_marker_label("ToxinDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.toxin_damage_applications_total)?;
        // SpectreGunship residual orbit fields (appended after toxin).
        xfer.xfer_marker_label("NextOrbitId")?;
        xfer.xfer_u32(&mut self.next_orbit_id)?;
        xfer.xfer_marker_label("OrbitFields")?;
        xfer_vec_default(
            xfer,
            &mut self.orbit_fields,
            crate::game_logic::special_power_strikes::HostSpectreOrbitField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                override_destination: Vec3::ZERO,
                gattling_target_position: Vec3::ZERO,
                position_to_shoot_at: Vec3::ZERO,
                ok_to_fire_howitzer_counter: 0,

                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                next_gattling_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
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
                howitzer_shell_death_lasered_ocl_applications: 0,
                howitzer_shell_death_generic_applications: 0,
                howitzer_shell_object_params_applications: 0,
                howitzer_shell_design_params_applications: 0,
                howitzer_shell_only_moving_down_applications: 0,
                howitzer_shell_model_draw_applications: 0,
                howitzer_shell_scale_applications: 0,
                howitzer_shell_shadow_applications: 0,
                howitzer_shell_geometry_applications: 0,
                howitzer_shell_max_health_applications: 0,
                howitzer_shell_loft_flight_applications: 0,
                howitzer_shell_last_loft_height: 0.0,
                howitzer_shell_loft_height_die_applications: 0,
                howitzer_shell_locomotor_template_applications: 0,
                howitzer_shell_damage_fx_applications: 0,
                howitzer_shell_thing_factory_spawn_applications: 0,
                howitzer_gun_aim_params_applications: 0,
                howitzer_gun_fire_params_applications: 0,
                howitzer_gun_anti_params_applications: 0,
                gattling_gun_params_applications: 0,
                gattling_rof_mean_applications: 0,
                gattling_rof_fast_applications: 0,
                gunship_position: None,
            },
        )?;
        xfer.xfer_marker_label("OrbitFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.orbit_fields_spawned_total)?;
        xfer.xfer_marker_label("OrbitDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.orbit_damage_applications_total)?;
        // ParticleCannon residual continuous beam fields (appended after orbit).
        xfer.xfer_marker_label("NextBeamId")?;
        xfer.xfer_u32(&mut self.next_beam_id)?;
        xfer.xfer_marker_label("BeamFields")?;
        xfer_vec_default(
            xfer,
            &mut self.beam_fields,
            crate::game_logic::special_power_strikes::HostParticleBeamField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                object_id: None,
                connector_object_ids: Vec::new(),
                position: Vec3::ZERO,
                source_position: Vec3::ZERO,
                source_axis_set: false,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                pulses_made: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                last_swath_position: Vec3::ZERO,
                max_swath_offset: 0.0,
                swath_applications: 0,
                next_scorch_frame: 0,
                scorch_marks_made: 0,
                reveal_applications: 0,
                ground_hit_fx_applications: 0,
                peak_width_scalar: 0.0,
                last_damage_radius: 0.0,
                last_width_scalar: 0.0,
                trough_width_scalar: 1.0,
                decay_samples: 0,
                last_scorch_position: Vec3::ZERO,
                last_scorch_radius: 0.0,
                manual_target_mode: false,
                override_destination: Vec3::ZERO,
                current_target_position: Vec3::ZERO,
                last_driving_click_frame: 0,
                second_last_driving_click_frame: 0,
                last_drive_update_frame: 0,
                manual_drive_distance_total: 0.0,
                manual_drive_applications: 0,
                fast_drive_applications: 0,
                scripted_waypoint_mode: false,
                next_dest_waypoint_id: 0,
                outer_node_systems_created: 0,
                connector_lasers_created: 0,
                laser_base_flare_created: 0,
                ground_to_orbit_laser_created: 0,
                status: crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                outer_intensity: crate::game_logic::special_power_strikes::ParticleIntensity::None,
                connector_intensity:
                    crate::game_logic::special_power_strikes::ParticleIntensity::None,
                laser_base_intensity:
                    crate::game_logic::special_power_strikes::ParticleIntensity::None,
                beam_launch_fx_applications: 0,
                next_launch_fx_frame: 0,
                postfire_applications: 0,
                packing_applications: 0,
                intensity_transitions: 0,
                connector_flare_created: 0,
                peak_outer_beam_draw_width: 0.0,
                last_outer_beam_draw_width: 0.0,
                peak_retail_laser_radius: 0.0,
                last_retail_laser_radius: 0.0,
                peak_retail_damage_radius: 0.0,
                last_retail_damage_radius: 0.0,
                orbital_laser_draw_params_armed: 0,
                connector_outer_beam_width_armed: 0,
                num_beams_armed: 0,
                tiling_scalar_armed: 0,
                last_scroll_uv: 0.0,
                peak_abs_scroll_uv: 0.0,
                scroll_uv_samples: 0,
                soft_edge_samples: 0,
                peak_soft_edge_outer_width: 0.0,
                last_soft_edge_outer_width: 0.0,
                last_soft_edge_outer_alpha: 0.0,
                last_soft_edge_tile_factor: 0.0,
                soft_edge_color_armed: 0,
                soft_edge_premul_samples: 0,
                last_soft_edge_premul_outer_r: 0.0,
                connector_soft_edge_premul_samples: 0,
                last_connector_soft_edge_premul_outer_r: 0.0,
                orbital_kindof_immobile_armed: 0,
                orbital_segments_armed: 0,
                orbital_arc_height_armed: 0,
                connector_kindof_immobile_armed: 0,
                connector_segments_armed: 0,
                connector_max_intensity_fade_armed: 0,
                connector_tile_no_armed: 0,
                outer_node_bone_layout_applications: 0,
                last_outer_node_bone_position: Vec3::ZERO,
                connector_bone_layout_applications: 0,
                connector_soft_edge_armed: 0,
                peak_connector_soft_edge_outer_width: 0.0,
                connector_laser_segments_created: 0,
                last_connector_segment_start: Vec3::ZERO,
                last_connector_segment_end: Vec3::ZERO,
                medium_connector_soft_edge_armed: 0,
                peak_medium_connector_soft_edge_outer_width: 0.0,
                orbital_vision_shroud_armed: 0,
                last_orbital_vision_range: 0.0,
                last_orbital_shroud_clearing_range: 0.0,
                laser_update_init_applications: 0,
                laser_update_dirty: false,
                laser_update_growth_frames: 0,
                laser_update_current_width_scalar: 0.0,
                laser_update_widening: false,
                laser_update_decaying: false,
                last_laser_update_start: Vec3::ZERO,
                last_laser_update_end: Vec3::ZERO,
                last_laser_update_drawable_mid: Vec3::ZERO,
                last_laser_update_radius: 0.0,
                ground_annihilation_audio_applications: 0,
                firing_to_pack_audio_applications: 0,
                sound_residual_pack_armed: 0,
                scorch_scalar_pack_armed: 0,
                outer_node_flare_pack_armed: 0,
                death_pack_armed: 0,
                start_decay_frame: 0,
            },
        )?;
        xfer.xfer_marker_label("BeamFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.beam_fields_spawned_total)?;
        xfer.xfer_marker_label("BeamObjectsSpawned")?;
        xfer.xfer_u32(&mut self.beam_objects_spawned)?;
        xfer.xfer_marker_label("BeamDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.beam_damage_applications_total)?;
        // Particle Uplink DamagePulseRemnant trail residual (appended after beam).
        xfer.xfer_marker_label("NextRemnantId")?;
        xfer.xfer_u32(&mut self.next_remnant_id)?;
        xfer.xfer_marker_label("RemnantFields")?;
        xfer_vec_default(
            xfer,
            &mut self.remnant_fields,
            crate::game_logic::special_power_strikes::HostParticleRemnantField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                object_id: None,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_beam_id: 0,
                parent_strike_id: 0,
                remnant_object_params_applications: 0,
                remnant_fire_deletion_applications: 0,
                remnant_immortal_body_applications: 0,
                remnant_thing_factory_spawn_applications: 0,
            },
        )?;
        xfer.xfer_marker_label("RemnantFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.remnant_fields_spawned_total)?;
        xfer.xfer_marker_label("RemnantObjectsSpawned")?;
        xfer.xfer_u32(&mut self.remnant_objects_spawned)?;
        xfer.xfer_marker_label("RemnantDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.remnant_damage_applications_total)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostParticleRemnantField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostParticleRemnantField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("ObjectId")?;
        xfer_option(xfer, &mut self.object_id, ObjectId(0))?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentBeamId")?;
        xfer.xfer_u32(&mut self.parent_beam_id)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // TrailRemnant KindOf / ImmortalBody residual (appended).
        xfer.xfer_marker_label("RemnantObjectParamsApplications")?;
        xfer.xfer_u32(&mut self.remnant_object_params_applications)?;
        // TrailRemnant FireWeaponUpdate + DeletionUpdate residual (appended).
        xfer.xfer_marker_label("RemnantFireDeletionApplications")?;
        xfer.xfer_u32(&mut self.remnant_fire_deletion_applications)?;
        // TrailRemnant ImmortalBody health-floor residual (appended).
        xfer.xfer_marker_label("RemnantImmortalBodyApplications")?;
        xfer.xfer_u32(&mut self.remnant_immortal_body_applications)?;
        // Wave 74: TrailRemnant ThingFactory spawn bookkeeping residual.
        xfer.xfer_marker_label("RemnantThingFactorySpawnApplications")?;
        xfer.xfer_u32(&mut self.remnant_thing_factory_spawn_applications)?;
        Ok(())
    }
}

impl XferData for CombatParticleKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            CombatParticleKind::DeathExplosion => 0u32,
            CombatParticleKind::DeathSmoke => 1,
            CombatParticleKind::WeaponMuzzleFlash => 2,
            CombatParticleKind::WeaponImpact => 3,
            CombatParticleKind::DeathBurn => 4,
            CombatParticleKind::DeathPoison => 5,
            CombatParticleKind::DeathLaser => 6,
            CombatParticleKind::ProjectileExhaust => 7,
            CombatParticleKind::ParticleSysBone => 8,
            CombatParticleKind::BodyFire => 9,
            CombatParticleKind::BodySmoke => 10,
            CombatParticleKind::DisableFx => 11,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => CombatParticleKind::DeathExplosion,
            1 => CombatParticleKind::DeathSmoke,
            2 => CombatParticleKind::WeaponMuzzleFlash,
            3 => CombatParticleKind::WeaponImpact,
            4 => CombatParticleKind::DeathBurn,
            5 => CombatParticleKind::DeathPoison,
            6 => CombatParticleKind::DeathLaser,
            7 => CombatParticleKind::ProjectileExhaust,
            8 => CombatParticleKind::ParticleSysBone,
            9 => CombatParticleKind::BodyFire,
            10 => CombatParticleKind::BodySmoke,
            11 => CombatParticleKind::DisableFx,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid CombatParticleKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for CombatParticleSystemEntry {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatParticleSystemEntry")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SourceObject")?;
        xfer_option(xfer, &mut self.source_object, ObjectId(0))?;
        xfer.xfer_marker_label("TargetObject")?;
        xfer_option(xfer, &mut self.target_object, ObjectId(0))?;
        xfer.xfer_marker_label("SpawnedFrame")?;
        xfer.xfer_u32(&mut self.spawned_frame)?;
        xfer.xfer_marker_label("Active")?;
        xfer.xfer_bool(&mut self.active)?;
        xfer.xfer_marker_label("ClientSystemId")?;
        // Option<u32> residual — client rebind may drop this after load.
        let mut has_client = self.client_system_id.is_some();
        xfer.xfer_bool(&mut has_client)?;
        if has_client {
            let mut id = self.client_system_id.unwrap_or(0);
            xfer.xfer_u32(&mut id)?;
            self.client_system_id = Some(id);
        } else {
            self.client_system_id = None;
        }
        xfer.xfer_marker_label("FxListName")?;
        xfer.xfer_string(&mut self.fx_list_name)?;
        Ok(())
    }
}

impl XferData for CombatParticleRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatParticleRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Systems")?;
        xfer_vec_default(
            xfer,
            &mut self.systems,
            CombatParticleSystemEntry {
                id: 0,
                kind: CombatParticleKind::DeathExplosion,
                template_name: String::new(),
                position: Vec3::ZERO,
                source_object: None,
                target_object: None,
                spawned_frame: 0,
                active: false,
                client_system_id: None,
                fx_list_name: String::new(),
                ocl_list_name: String::new(),
                attach_offset: Vec3::ZERO,
            },
        )?;
        Ok(())
    }
}

impl XferData for HostUpgradeKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostUpgradeKind::CaptureBuilding => 0u32,
            HostUpgradeKind::FlashBangGrenade => 1,
            HostUpgradeKind::TowMissile => 2,
            HostUpgradeKind::SupplyLines => 3,
            HostUpgradeKind::NeutronShells => 5,
            HostUpgradeKind::Other => 4,
            HostUpgradeKind::BunkerBusters => 6,
            HostUpgradeKind::ComancheRocketPods => 7,
            HostUpgradeKind::SentryDroneGun => 8,
            HostUpgradeKind::Camouflage => 9,
            HostUpgradeKind::CompositeArmor => 10,
            HostUpgradeKind::WorkerShoes => 11,
            HostUpgradeKind::NuclearTanks => 12,
            HostUpgradeKind::BoobyTrap => 13,
            HostUpgradeKind::AnthraxGamma => 14,
            HostUpgradeKind::CamoNetting => 15,
            HostUpgradeKind::SuicideBomb => 16,
            HostUpgradeKind::AdvancedControlRods => 17,
            HostUpgradeKind::SubliminalMessaging => 18,
            HostUpgradeKind::ScorpionRocket => 19,
            HostUpgradeKind::ApRockets => 20,
            HostUpgradeKind::LaserMissiles => 21,
            HostUpgradeKind::Nationalism => 22,
            HostUpgradeKind::Fanaticism => 46,
            HostUpgradeKind::ChainGuns => 23,
            HostUpgradeKind::UraniumShells => 24,
            HostUpgradeKind::BlackNapalm => 25,
            HostUpgradeKind::ApBullets => 26,
            HostUpgradeKind::AnthraxBeta => 27,
            HostUpgradeKind::ToxinShells => 28,
            HostUpgradeKind::AdvancedTraining => 29,
            HostUpgradeKind::TacticalNukeMig => 30,
            HostUpgradeKind::DroneArmor => 31,
            HostUpgradeKind::AircraftArmor => 32,
            HostUpgradeKind::ChinaMines => 33,
            HostUpgradeKind::EmpMines => 34,
            HostUpgradeKind::FortifiedStructure => 35,
            HostUpgradeKind::Radar => 36,
            HostUpgradeKind::RadarVanScan => 37,
            HostUpgradeKind::ChemicalSuits => 38,
            HostUpgradeKind::Moab => 39,
            HostUpgradeKind::SatelliteHack => 40,
            HostUpgradeKind::Countermeasures => 41,
            HostUpgradeKind::SlaveDrone => 42,
            HostUpgradeKind::CashBounty => 43,
            HostUpgradeKind::HelixNapalmBomb => 44,
            HostUpgradeKind::HelixNukeBomb => 45,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostUpgradeKind::CaptureBuilding,
            1 => HostUpgradeKind::FlashBangGrenade,
            2 => HostUpgradeKind::TowMissile,
            3 => HostUpgradeKind::SupplyLines,
            4 => HostUpgradeKind::Other,
            5 => HostUpgradeKind::NeutronShells,
            6 => HostUpgradeKind::BunkerBusters,
            7 => HostUpgradeKind::ComancheRocketPods,
            8 => HostUpgradeKind::SentryDroneGun,
            9 => HostUpgradeKind::Camouflage,
            10 => HostUpgradeKind::CompositeArmor,
            11 => HostUpgradeKind::WorkerShoes,
            12 => HostUpgradeKind::NuclearTanks,
            13 => HostUpgradeKind::BoobyTrap,
            14 => HostUpgradeKind::AnthraxGamma,
            15 => HostUpgradeKind::CamoNetting,
            16 => HostUpgradeKind::SuicideBomb,
            17 => HostUpgradeKind::AdvancedControlRods,
            18 => HostUpgradeKind::SubliminalMessaging,
            19 => HostUpgradeKind::ScorpionRocket,
            20 => HostUpgradeKind::ApRockets,
            21 => HostUpgradeKind::LaserMissiles,
            22 => HostUpgradeKind::Nationalism,
            23 => HostUpgradeKind::ChainGuns,
            24 => HostUpgradeKind::UraniumShells,
            25 => HostUpgradeKind::BlackNapalm,
            26 => HostUpgradeKind::ApBullets,
            27 => HostUpgradeKind::AnthraxBeta,
            28 => HostUpgradeKind::ToxinShells,
            29 => HostUpgradeKind::AdvancedTraining,
            30 => HostUpgradeKind::TacticalNukeMig,
            31 => HostUpgradeKind::DroneArmor,
            32 => HostUpgradeKind::AircraftArmor,
            33 => HostUpgradeKind::ChinaMines,
            34 => HostUpgradeKind::EmpMines,
            35 => HostUpgradeKind::FortifiedStructure,
            36 => HostUpgradeKind::Radar,
            37 => HostUpgradeKind::RadarVanScan,
            38 => HostUpgradeKind::ChemicalSuits,
            39 => HostUpgradeKind::Moab,
            40 => HostUpgradeKind::SatelliteHack,
            41 => HostUpgradeKind::Countermeasures,
            42 => HostUpgradeKind::SlaveDrone,
            43 => HostUpgradeKind::CashBounty,
            44 => HostUpgradeKind::HelixNapalmBomb,
            45 => HostUpgradeKind::HelixNukeBomb,
            46 => HostUpgradeKind::Fanaticism,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostUpgradeKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostUpgradePhase {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostUpgradePhase::Queued => 0u32,
            HostUpgradePhase::Completed => 1,
            HostUpgradePhase::Cancelled => 2,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostUpgradePhase::Queued,
            1 => HostUpgradePhase::Completed,
            2 => HostUpgradePhase::Cancelled,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostUpgradePhase discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostUpgradeResearch {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostUpgradeResearch")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Name")?;
        self.name.xfer(xfer)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("QueueFrame")?;
        xfer.xfer_u32(&mut self.queue_frame)?;
        xfer.xfer_marker_label("CompleteFrame")?;
        xfer.xfer_u32(&mut self.complete_frame)?;
        xfer.xfer_marker_label("Phase")?;
        self.phase.xfer(xfer)?;
        xfer.xfer_marker_label("UnitsAffected")?;
        xfer.xfer_u32(&mut self.units_affected)?;
        xfer.xfer_marker_label("SourceObject")?;
        xfer_option(xfer, &mut self.source_object, ObjectId(0))?;
        // Wave 79: cost/time residual application bookkeeping (appended).
        xfer.xfer_marker_label("BuildCostPaid")?;
        xfer.xfer_u32(&mut self.build_cost_paid)?;
        xfer.xfer_marker_label("RetailResearchFrames")?;
        xfer.xfer_u32(&mut self.retail_research_frames)?;
        xfer.xfer_marker_label("ResidualResearchFrames")?;
        xfer.xfer_u32(&mut self.residual_research_frames)?;
        Ok(())
    }
}

impl XferData for HostUpgradeRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostUpgradeRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Entries")?;
        xfer_vec_default(
            xfer,
            &mut self.entries,
            HostUpgradeResearch {
                id: 0,
                name: String::new(),
                kind: HostUpgradeKind::Other,
                team: Team::Neutral,
                player_id: 0,
                queue_frame: 0,
                complete_frame: 0,
                phase: HostUpgradePhase::Queued,
                units_affected: 0,
                source_object: None,
                build_cost_paid: 0,
                retail_research_frames: 0,
                residual_research_frames: 1,
            },
        )?;
        Ok(())
    }
}
