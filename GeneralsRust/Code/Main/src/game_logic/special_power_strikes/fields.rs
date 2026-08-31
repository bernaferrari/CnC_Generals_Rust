//! Radiation, toxin, neutron, Spectre orbit, Particle beam, and remnant field records.
use super::types::*;
use super::*;
/// C++ `SpecialPowerModule::createViewObject` residual (range 250 / 30-40s).
#[derive(Debug, Clone, PartialEq)]
pub struct HostViewObjectReveal {
    pub source_object: ObjectId,
    pub player_id: u32,
    pub position: Vec3,
    pub range: f32,
    pub spawn_frame: u32,
    pub expires_frame: u32,
    pub object_id: Option<ObjectId>,
    pub fow_reveal_ok: bool,
}

impl HostViewObjectReveal {
    pub fn duration_frames(&self) -> u32 {
        self.expires_frame.saturating_sub(self.spawn_frame)
    }
}

/// Residual radiation field spawned by NuclearMissile impact
/// (`OCL_NukeRadiationField` / `NukeRadiationFieldWeapon` residual).
/// Epicenter metadata for a NeutronMissileSlowDeath residual field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostNeutronSlowDeathMeta {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    pub parent_strike_id: u32,
    pub scorch_size: f32,
    pub fx_list: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostRadiationField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: crate::game_logic::Team,
    /// Host GameLogic ObjectId for NukeRadiationFieldWeapon residual.
    pub object_id: Option<ObjectId>,
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
    /// Honesty: radiation residual pack armed (SuspendFX / FireFX / OCL).
    #[serde(default)]
    pub radiation_residual_pack_armed: u32,
    /// Honesty: SuspendFXDelay residual applications.
    #[serde(default)]
    pub radiation_suspend_fx_applications: u32,
    /// Honesty: FireFX residual applications.
    #[serde(default)]
    pub radiation_fire_fx_applications: u32,
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
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    pub hits: Vec<HostRadiationDamageHit>,
}

/// Residual toxin / anthrax / scud poison field spawned by AnthraxBomb or
/// ScudStorm impact (`OCL_PoisonFieldAnthraxBomb` / `OCL_PoisonFieldLarge` residual).
fn default_toxin_object_template() -> String {
    ANTHRAX_TOXIN_OBJECT_NAME.to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostToxinField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: crate::game_logic::Team,
    /// Host GameLogic ObjectId for poison field residual object.
    pub object_id: Option<ObjectId>,
    /// ThingFactory template residual (PoisonFieldAnthraxBomb / PoisonFieldLarge).
    #[serde(default = "default_toxin_object_template")]
    pub object_template: String,
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
    /// Honesty: anthrax/scud poison residual pack armed (FireFX / DeathType / OCL).
    #[serde(default)]
    pub toxin_residual_pack_armed: u32,
    /// Honesty: poison FireFX residual applications.
    #[serde(default)]
    pub toxin_fire_fx_applications: u32,
    /// Honesty: DeathType / DamageType residual applications.
    #[serde(default)]
    pub toxin_damage_type_applications: u32,
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
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    pub hits: Vec<HostToxinDamageHit>,
    /// Weapon.ini DeathType residual (POISONED / BETA / GAMMA).
    pub death_type: crate::game_logic::host_usa_pilot::HostDeathType,
}

/// Residual Spectre orbit field spawned when gunship reaches target
/// (`SpectreGunshipUpdate` GUNSHIP_STATUS_ORBITING / `SpectreHowitzerGun` residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSpectreOrbitField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    /// C++ `m_overrideTargetDestination`. Clamped to AttackAreaRadius-Reticle
    /// from `position` (`m_initialTargetPosition`). Override clicks never
    /// rewrite `position`.
    #[serde(default)]
    pub override_destination: Vec3,
    /// C++ `m_gattlingTargetPosition`. Howitzer fires here after StrafingIncrement wind.
    #[serde(default)]
    pub gattling_target_position: Vec3,
    /// C++ `m_positionToShootAt` (reticle / AI-wide acquire).
    #[serde(default)]
    pub position_to_shoot_at: Vec3,
    /// C++ `m_okToFireHowitzerCounter`. Increments on-target, resets while winding.
    #[serde(default)]
    pub ok_to_fire_howitzer_counter: u32,

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
    /// Honesty: InstantDeath LASERED OCL residual applications (OCL_GenericMissileDisintegrate).
    #[serde(default)]
    pub howitzer_shell_death_lasered_ocl_applications: u32,
    /// Honesty: InstantDeath GENERIC residual applications (FX_GenericMissileDeath).
    #[serde(default)]
    pub howitzer_shell_death_generic_applications: u32,
    /// Honesty: KindOf / VisionRange / Armor residual applications.
    #[serde(default)]
    pub howitzer_shell_object_params_applications: u32,
    /// Honesty: TargetHeightIncludesStructures / InitialHealth / DisplayName residual.
    #[serde(default)]
    pub howitzer_shell_design_params_applications: u32,
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
    /// Honesty: shell loft flight residual applications (pad-safe delay path).
    #[serde(default)]
    pub howitzer_shell_loft_flight_applications: u32,
    /// Honesty: last shell loft height residual sample.
    #[serde(default)]
    pub howitzer_shell_last_loft_height: f32,
    /// Honesty: shell loft height-die residual applications.
    #[serde(default)]
    pub howitzer_shell_loft_height_die_applications: u32,
    /// Honesty: SpectreHowitzerShellLocomotor template residual applications.
    #[serde(default)]
    pub howitzer_shell_locomotor_template_applications: u32,
    /// Honesty: Armor DamageFX=None residual applications.
    #[serde(default)]
    pub howitzer_shell_damage_fx_applications: u32,
    /// Honesty: SpectreHowitzerShell ThingFactory residual spawn bookkeeping
    /// applications (Wave 74; shell spawn object pack ledger).
    #[serde(default)]
    pub howitzer_shell_thing_factory_spawn_applications: u32,
    /// Honesty: SpectreHowitzerGun AcceptableAimDelta/AttackRange residual applications.
    #[serde(default)]
    pub howitzer_gun_aim_params_applications: u32,
    /// Honesty: SpectreHowitzerGun fire residual (Delay/DamageType/FireFX/Clip/GroupPriority) applications.
    #[serde(default)]
    pub howitzer_gun_fire_params_applications: u32,
    /// Honesty: SpectreHowitzerGun anti residual applications (AntiAir*/ProjectileObject/Coast).
    #[serde(default)]
    pub howitzer_gun_anti_params_applications: u32,
    /// Honesty: SpectreGattlingGun anti/fire residual applications.
    #[serde(default)]
    pub gattling_gun_params_applications: u32,
    /// Honesty: ContinuousFire WeaponBonus MEAN ROF residual applications
    /// (ticks that used RATE_OF_FIRE 200% interval residual).
    #[serde(default)]
    pub gattling_rof_mean_applications: u32,
    /// Honesty: ContinuousFire WeaponBonus FAST ROF residual applications
    /// (ticks that used RATE_OF_FIRE 300% interval residual).
    #[serde(default)]
    pub gattling_rof_fast_applications: u32,
    /// Live gunship world position for `isFairDistanceFromShip` (hq-2ulfq).
    /// `None` until the host binds a living gunship — acquire then fail-closes.
    #[serde(default)]
    pub gunship_position: Option<Vec3>,
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

    /// C++ clamped reticle / howitzer aim (`m_overrideTargetDestination`).
    /// Falls back to the orbit epicenter when no override has been stored.
    pub fn override_aim(&self) -> Vec3 {
        if self.override_destination == Vec3::ZERO && self.position != Vec3::ZERO {
            self.position
        } else {
            self.override_destination
        }
    }

    /// C++ `setSpecialPowerOverridableDestination` + update split
    /// (SpectreGunshipUpdate.cpp:268-282, 400-439).
    ///
    /// The click steers the orbit CENTER (`m_initialTargetPosition`, host
    /// `position`) via satellite `aiMoveToPosition` — unclamped. The CLAMP
    /// applies only to the reticle (`m_overrideTargetDestination`, host
    /// `override_destination`) against that initial target with
    /// `constraintRadius = AttackAreaRadius - TargetingReticleRadius`.
    pub fn apply_override_destination(&mut self, destination: Vec3) {
        self.position = destination;
        self.override_destination = clamp_spectre_override_destination(
            self.position,
            destination,
            SPECTRE_ORBIT_RADIUS,
            SPECTRE_TARGETING_RETICLE_RADIUS,
        );
        // The gattling / howitzer aim chain follows the new orbit epicenter.
        self.gattling_target_position = destination;
        self.position_to_shoot_at = destination;
    }

    /// C++ lagged gattling / howitzer aim. Falls back to the orbit epicenter.
    pub fn gattling_aim(&self) -> Vec3 {
        if self.gattling_target_position == Vec3::ZERO && self.position != Vec3::ZERO {
            self.position
        } else {
            self.gattling_target_position
        }
    }

    /// C++ `m_positionToShootAt`. Falls back to the clamped reticle.
    pub fn shoot_at_aim(&self) -> Vec3 {
        if self.position_to_shoot_at == Vec3::ZERO && self.position != Vec3::ZERO {
            self.override_aim()
        } else {
            self.position_to_shoot_at
        }
    }

    /// C++ `m_okToFireHowitzerCounter > HowitzerFollowLag` (12f).
    pub fn howitzer_follow_ready(&self) -> bool {
        spectre_howitzer_follow_ready(self.ok_to_fire_howitzer_counter)
    }

    /// C++ howitzer-rate re-eval: `m_positionToShootAt = m_overrideTargetDestination`.
    pub fn refresh_position_to_shoot_at(&mut self) {
        self.position_to_shoot_at = self.override_aim();
    }

    /// C++ gattling wind: step toward shoot-at by StrafingIncrement, reset/inc lag.
    pub fn wind_gattling_aim(&mut self) {
        let (next, counter) = spectre_wind_gattling_aim(
            self.gattling_aim(),
            self.shoot_at_aim(),
            SPECTRE_STRAFING_INCREMENT,
            self.ok_to_fire_howitzer_counter,
        );
        self.gattling_target_position = next;
        self.ok_to_fire_howitzer_counter = counter;
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
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    pub hits: Vec<HostSpectreOrbitDamageHit>,
}

/// Residual Particle Uplink continuous beam field spawned when charge residual
/// completes (`ParticleUplinkCannonUpdate` STATUS_FIRING / TotalDamagePulses).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostParticleBeamField {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: crate::game_logic::Team,
    /// Host GameLogic ObjectId for ParticleUplinkCannon_OrbitalLaser residual.
    pub object_id: Option<ObjectId>,
    /// Host GameLogic ObjectIds for Medium/Intense connector lasers residual.
    #[serde(default)]
    pub connector_object_ids: Vec<ObjectId>,
    /// Click / initial target epicenter residual (swath walks around this).
    pub position: Vec3,
    /// PUC building world position for SwathOfDeath axis (C++ `me->getPosition`).
    #[serde(default)]
    pub source_position: Vec3,
    /// True when [`source_position`] is the live cannon (not an unset default).
    #[serde(default)]
    pub source_axis_set: bool,
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
    /// Honesty: last residual damage radius used (WidthGrow × retail 44.2).
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
    /// [`override_destination`] instead of SwathOfDeath S-curve. Retail human
    /// fire (`!COMMAND_FIRED_BY_SCRIPT`) starts in this mode so the beam holds
    /// the click. Script/AI residual and direct `spawn_beam_field` stay swath
    /// until an override is applied.
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
    /// C++ `m_scriptedWaypointMode` — leftover chain drive, not SwathOfDeath.
    #[serde(default)]
    pub scripted_waypoint_mode: bool,
    /// C++ `m_nextDestWaypointID` (leftover terrain waypoint id).
    #[serde(default)]
    pub next_dest_waypoint_id: u32,
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
    /// Honesty: soft-edge RGB innerAlpha premultiply residual samples.
    #[serde(default)]
    pub soft_edge_premul_samples: u32,
    /// Honesty: last soft-edge premul outer red residual.
    #[serde(default)]
    pub last_soft_edge_premul_outer_r: f32,
    /// Honesty: connector soft-edge RGB innerAlpha premul residual samples.
    #[serde(default)]
    pub connector_soft_edge_premul_samples: u32,
    /// Honesty: last intense connector soft-edge premul outer red residual.
    #[serde(default)]
    pub last_connector_soft_edge_premul_outer_r: f32,
    /// Honesty: OrbitalLaser KindOf IMMOBILE residual armed.
    #[serde(default)]
    pub orbital_kindof_immobile_armed: u32,
    /// Honesty: W3DLaserDraw Segments residual armed (default 1).
    #[serde(default)]
    pub orbital_segments_armed: u32,
    /// Honesty: W3DLaserDraw ArcHeight residual armed (default 0).
    #[serde(default)]
    pub orbital_arc_height_armed: u32,
    /// Honesty: connector KindOf IMMOBILE residual armed.
    #[serde(default)]
    pub connector_kindof_immobile_armed: u32,
    /// Honesty: connector W3DLaserDraw Segments residual armed (default 1).
    #[serde(default)]
    pub connector_segments_armed: u32,
    /// Honesty: connector MaxIntensity/FadeLifetime residual defaults armed.
    #[serde(default)]
    pub connector_max_intensity_fade_armed: u32,
    /// Honesty: connector Tile=No residual armed.
    #[serde(default)]
    pub connector_tile_no_armed: u32,
    /// Honesty: outer-node bone layout residual positions computed.
    #[serde(default)]
    pub outer_node_bone_layout_applications: u32,
    /// Honesty: last outer-node bone residual position (FX01).
    #[serde(default)]
    pub last_outer_node_bone_position: Vec3,
    /// Honesty: connector bone residual position applications.
    #[serde(default)]
    pub connector_bone_layout_applications: u32,
    /// Honesty: intense connector soft-edge residual armed at STATUS_FIRING.
    #[serde(default)]
    pub connector_soft_edge_armed: u32,
    /// Honesty: peak intense connector soft-edge outer width residual.
    #[serde(default)]
    pub peak_connector_soft_edge_outer_width: f32,
    /// Honesty: connector laser segments residual (outer-node → connector).
    #[serde(default)]
    pub connector_laser_segments_created: u32,
    /// Honesty: last connector laser segment start residual (outer node 0).
    #[serde(default)]
    pub last_connector_segment_start: Vec3,
    /// Honesty: last connector laser segment end residual (connector bone).
    #[serde(default)]
    pub last_connector_segment_end: Vec3,
    /// Honesty: medium connector soft-edge residual armed (POSTFIRE Medium intensity).
    #[serde(default)]
    pub medium_connector_soft_edge_armed: u32,
    /// Honesty: peak medium connector soft-edge outer width residual.
    #[serde(default)]
    pub peak_medium_connector_soft_edge_outer_width: f32,
    /// Honesty: OrbitalLaser VisionRange / ShroudClearing residual armed.
    #[serde(default)]
    pub orbital_vision_shroud_armed: u32,
    /// Honesty: last VisionRange residual sample.
    #[serde(default)]
    pub last_orbital_vision_range: f32,
    /// Honesty: last ShroudClearingRange residual sample.
    #[serde(default)]
    pub last_orbital_shroud_clearing_range: f32,
    /// Honesty: LaserUpdate initLaser residual applications (ground-to-orbit + orbit-to-target).
    #[serde(default)]
    pub laser_update_init_applications: u32,
    /// Honesty: LaserUpdate m_dirty residual after init/update.
    #[serde(default)]
    pub laser_update_dirty: bool,
    /// Honesty: LaserUpdate sizeDeltaFrames residual (WidthGrow frames at init).
    #[serde(default)]
    pub laser_update_growth_frames: u32,
    /// Honesty: LaserUpdate m_currentWidthScalar residual sample.
    #[serde(default)]
    pub laser_update_current_width_scalar: f32,
    /// Honesty: LaserUpdate widening residual active.
    #[serde(default)]
    pub laser_update_widening: bool,
    /// Honesty: LaserUpdate decaying residual active (POSTFIRE setDecayFrames).
    #[serde(default)]
    pub laser_update_decaying: bool,
    /// Honesty: last LaserUpdate start residual (orbit-to-target start = target+500).
    #[serde(default)]
    pub last_laser_update_start: Vec3,
    /// Honesty: last LaserUpdate end residual (orbit-to-target end = target).
    #[serde(default)]
    pub last_laser_update_end: Vec3,
    /// Honesty: last LaserUpdate drawable midpoint residual.
    #[serde(default)]
    pub last_laser_update_drawable_mid: Vec3,
    /// Honesty: last LaserUpdate getCurrentLaserRadius residual.
    #[serde(default)]
    pub last_laser_update_radius: f32,
    /// Honesty: GroundAnnihilationSoundLoop residual applications (STATUS_FIRING).
    #[serde(default)]
    pub ground_annihilation_audio_applications: u32,
    /// Honesty: FiringToPackSoundLoop residual applications (STATUS_FIRING).
    #[serde(default)]
    pub firing_to_pack_audio_applications: u32,
    /// Honesty: full PUC sound residual pack armed at beam spawn (names + FX).
    #[serde(default)]
    pub sound_residual_pack_armed: u32,
    /// Honesty: ScorchMarkScalar residual pack armed (scorch radius formula).
    #[serde(default)]
    pub scorch_scalar_pack_armed: u32,
    /// Honesty: OuterNodes Light/Medium/Intense + LaserBase + connector name pack
    /// armed at STATUS_FIRING residual (FactionBuilding.ini particle systems).
    #[serde(default)]
    pub outer_node_flare_pack_armed: u32,
    /// Honesty: PUC SlowDeath / InstantDeath residual pack armed (building death
    /// design params; fail-closed vs live SlowDeathBehavior Object die).
    #[serde(default)]
    pub death_pack_armed: u32,
    /// C++ `m_startDecayFrame`. 0 means spawn + TotalFiringTime (legacy snapshot).
    #[serde(default)]
    pub start_decay_frame: u32,
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

    /// C++ `m_startDecayFrame` (0 on legacy snapshots → spawn + TotalFiringTime).
    pub fn live_decay_start_frame(&self) -> u32 {
        if self.start_decay_frame == 0 {
            particle_decay_start_frame(self.spawn_frame)
        } else {
            self.start_decay_frame
        }
    }

    /// C++ abort: `m_startDecayFrame = now`, then WidthGrow decay tail.
    pub fn begin_abort_decay(&mut self, now: u32) {
        if self.live_decay_start_frame() > now {
            self.start_decay_frame = now;
            self.expires_frame = now.saturating_add(PARTICLE_WIDTH_GROW_FRAMES);
        }
    }

    /// True when a damage pulse residual is due.
    ///
    /// Pulses run across the full orbital lifetime (TotalFiringTime + WidthGrow
    /// decay tail), matching C++ `now <= orbitalDeathFrame`. They stop once
    /// TotalDamagePulses is reached, or immediately after an early abort
    /// (`start_decay_frame = now` from disable/EMP).
    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        let aborted_early = self.start_decay_frame != 0
            && self.start_decay_frame < particle_decay_start_frame(self.spawn_frame);
        !self.is_expired(current_frame)
            && self.pulses_made < PARTICLE_BEAM_TOTAL_PULSES
            && current_frame >= self.next_tick_frame
            && !aborted_early
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
        let soft_w = particle_orbital_soft_edge_width(outer_idx, self.spawn_frame, current_frame);
        self.last_soft_edge_outer_width = soft_w;
        if soft_w > self.peak_soft_edge_outer_width {
            self.peak_soft_edge_outer_width = soft_w;
        }
        self.last_soft_edge_outer_alpha = particle_orbital_soft_edge_alpha(outer_idx);
        // Unit-length outer cylinder tile-factor residual (aspect × TilingScalar).
        self.last_soft_edge_tile_factor =
            particle_orbital_soft_edge_tile_factor(1.0, soft_w.max(f32::EPSILON));
        self.soft_edge_samples = self.soft_edge_samples.saturating_add(1);
        // Soft-edge RGB innerAlpha premultiply residual (W3DLaserDraw channel delta).
        let (_pr, _pg, _pb, _) = particle_orbital_soft_edge_color_premul(0);
        let (or_p, _og_p, _ob_p, _) = particle_orbital_soft_edge_color_premul(outer_idx);
        self.last_soft_edge_premul_outer_r = or_p;
        self.soft_edge_premul_samples = self.soft_edge_premul_samples.saturating_add(1);
        // Intense connector soft-edge RGB premul residual (W3DLaserDraw channel delta).
        let conn_idx = PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS.saturating_sub(1);
        let (cr_p, _cg_p, _cb_p, _) = particle_connector_intense_soft_edge_color_premul(conn_idx);
        self.last_connector_soft_edge_premul_outer_r = cr_p;
        self.connector_soft_edge_premul_samples =
            self.connector_soft_edge_premul_samples.saturating_add(1);
        // LaserUpdate client residual: currentWidthScalar widen/decay samples.
        // Retail createOrbitToTargetLaser(sizeDelta = WidthGrow) then setDecayFrames
        // at POSTFIRE. Host residual mirrors the same scalar schedule.
        let elapsed = current_frame.saturating_sub(self.spawn_frame);
        let decay_start = particle_decay_start_frame(self.spawn_frame);
        if current_frame >= decay_start {
            let decay_elapsed = current_frame.saturating_sub(decay_start);
            self.laser_update_current_width_scalar =
                laser_update_width_scalar_decay(decay_elapsed, PARTICLE_WIDTH_GROW_FRAMES);
            self.laser_update_widening = false;
            self.laser_update_decaying = true;
        } else {
            self.laser_update_current_width_scalar =
                laser_update_width_scalar_widen(elapsed, PARTICLE_WIDTH_GROW_FRAMES);
            self.laser_update_widening = elapsed < PARTICLE_WIDTH_GROW_FRAMES;
            self.laser_update_decaying = false;
        }
        self.laser_update_dirty = true;
        self.last_laser_update_radius =
            laser_update_current_radius(self.laser_update_current_width_scalar);
        if current_frame > decay_start && current_frame < self.expires_frame {
            self.decay_samples = self.decay_samples.saturating_add(1);
            if width_scalar < self.trough_width_scalar {
                self.trough_width_scalar = width_scalar;
            }
        }
    }

    /// Residual damage / scorch epicenter for the current beam mode.
    ///
    /// Manual mode uses live `current_target_position`. Swath mode walks the
    /// S-curve; when the cannon position is bound, leftover/C++ rotates that
    /// offset onto building→target instead of world +X.
    pub fn residual_epicenter(&self, pulse_index: u32) -> Vec3 {
        if self.manual_target_mode || self.scripted_waypoint_mode {
            self.current_target_position
        } else if self.source_axis_set {
            particle_swath_epicenter_along(self.source_position, self.position, pulse_index)
        } else {
            particle_swath_epicenter(self.position, pulse_index)
        }
    }

    /// Stamp the live cannon position so SwathOfDeath rotates onto cannon→click.
    pub fn bind_source_axis(&mut self, building: Vec3) {
        self.source_position = building;
        self.source_axis_set = true;
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
    pub source_team: crate::game_logic::Team,
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
    pub source_team: crate::game_logic::Team,
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
    pub source_team: crate::game_logic::Team,
    /// Host GameLogic ObjectId for ParticleUplinkCannonTrailRemnant residual.
    pub object_id: Option<ObjectId>,
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
    /// Honesty: TrailRemnant KindOf / ImmortalBody residual applications.
    #[serde(default)]
    pub remnant_object_params_applications: u32,
    /// Honesty: TrailRemnant FireWeaponUpdate + DeletionUpdate residual applications.
    #[serde(default)]
    pub remnant_fire_deletion_applications: u32,
    /// Honesty: ImmortalBody health-floor residual applications.
    #[serde(default)]
    pub remnant_immortal_body_applications: u32,
    /// Honesty: TrailRemnant ThingFactory residual spawn bookkeeping
    /// applications (Wave 74; ImmortalBody/DeletionUpdate pack ledger).
    #[serde(default)]
    pub remnant_thing_factory_spawn_applications: u32,
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
    pub source_team: crate::game_logic::Team,
    pub position: Vec3,
    pub hits: Vec<HostParticleRemnantDamageHit>,
}
