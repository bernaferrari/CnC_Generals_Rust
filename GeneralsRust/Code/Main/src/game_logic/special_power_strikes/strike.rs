//! Queued host strike records, impact plans, and ParticleCannon charge-status apply.
use super::types::*;
use super::*;
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
    pub source_team: crate::game_logic::Team,
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
    /// A10 science-tier FormationSize residual (1/2/3 jets).
    /// Ignored for non-A10 kinds. Default Level1.
    #[serde(default)]
    pub a10_tier: A10StrikeScienceTier,
    /// Honesty: A10 FormationSize residual applications at queue.
    #[serde(default)]
    pub a10_formation_size_applications: u32,
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
    /// Honesty: PoweringUpSoundLoop residual applications (STATUS_CHARGING).
    #[serde(default)]
    pub particle_powerup_audio_applications: u32,
    /// Honesty: UnpackToIdleSoundLoop residual applications (STATUS_PREPARING).
    #[serde(default)]
    pub particle_unpack_audio_applications: u32,
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
    /// Honesty: ThrustRoll / ThrustWobble residual applications.
    #[serde(default)]
    pub scud_thrust_wobble_applications: u32,
    /// Honesty: last thrust wobble residual sample.
    #[serde(default)]
    pub scud_last_thrust_wobble: f32,
    /// Honesty: peak |thrust wobble| residual.
    #[serde(default)]
    pub scud_peak_abs_thrust_wobble: f32,
    /// Honesty: Geometry residual applications (Cylinder / IsSmall / major+height / mass).
    #[serde(default)]
    pub scud_geometry_applications: u32,
    /// Honesty: VisionRange / KindOf / Armor / TransportSlot residual applications.
    #[serde(default)]
    pub scud_object_params_applications: u32,
    /// Honesty: MissileAIUpdate residual applications (TryToFollow/Fuel/DistTurning).
    #[serde(default)]
    pub scud_missile_ai_applications: u32,
    /// Honesty: FireWeaponWhenDead death-weapon matrix residual applications.
    #[serde(default)]
    pub scud_fire_weapon_when_dead_applications: u32,
    /// Honesty: InitialHealth / EditorSorting / OkToChangeModelColor residual applications.
    #[serde(default)]
    pub scud_body_draw_params_applications: u32,
    /// Honesty: Locomotor Surfaces/Appearance/AllowAirborne/Braking residual applications.
    #[serde(default)]
    pub scud_locomotor_appearance_applications: u32,
    /// Honesty: DestroyDie + Locomotor template name + Armor DamageFX residual applications.
    #[serde(default)]
    pub scud_destroy_die_locomotor_name_applications: u32,
    /// Honesty: DeathWeapon FireOCL PoisonField residual applications.
    #[serde(default)]
    pub scud_death_fire_ocl_applications: u32,
    /// Honesty: Locomotor SpeedDamaged/MinSpeed/MaxThrustAngle residual applications.
    #[serde(default)]
    pub scud_locomotor_speed_table_applications: u32,
    /// Honesty: DeathWeapon Primary/Secondary damage table residual applications.
    #[serde(default)]
    pub scud_death_damage_table_applications: u32,
    /// Honesty: ScudStormWeapon launch residual applications (Clip/Scatter/AutoReload).
    #[serde(default)]
    pub scud_weapon_launch_applications: u32,
    /// Honesty: ScudStormWeapon special residual applications (unused Primary/Speed/PreAttackType).
    #[serde(default)]
    pub scud_weapon_special_applications: u32,
    /// Honesty: MissileAIUpdate defaults residual applications.
    #[serde(default)]
    pub scud_missile_ai_defaults_applications: u32,
    /// Honesty: ScudStormMissile ThingFactory residual spawn bookkeeping
    /// applications (Wave 74; impact-time object pack ledger).
    #[serde(default)]
    pub scud_thing_factory_spawn_applications: u32,
    /// CarpetBomb faction residual (USA15 / AirF12 / China10). Default America.
    #[serde(default)]
    pub carpet_tier: CarpetBombFactionTier,
    /// Honesty: CarpetBomb residual pack armed at queue (Wave 56).
    #[serde(default)]
    pub carpet_residual_pack_armed: u32,
    /// Honesty: AmericaJetB52 PreferredHeight residual applications.
    #[serde(default)]
    pub carpet_preferred_height_applications: u32,
    /// Honesty: DropDelay stagger residual applications.
    #[serde(default)]
    pub carpet_drop_delay_applications: u32,
    /// Honesty: DropVariance residual applications.
    #[serde(default)]
    pub carpet_drop_variance_applications: u32,
    /// Honesty: bomb-count / line-length residual applications.
    #[serde(default)]
    pub carpet_bomb_count_applications: u32,
    /// Honesty: FireFX FX_CarpetBomb residual applications (per bomb wave).
    #[serde(default)]
    pub carpet_fire_fx_applications: u32,
    /// Honesty: DeliveryDistance residual applications.
    #[serde(default)]
    pub carpet_delivery_distance_applications: u32,
    /// Honesty: ArtilleryBarrage residual pack armed at queue (Wave 56).
    #[serde(default)]
    pub artillery_residual_pack_armed: u32,
    /// Honesty: ChinaArtilleryCannon transport residual applications.
    #[serde(default)]
    pub artillery_cannon_transport_applications: u32,
    /// Honesty: FormationSize residual applications.
    #[serde(default)]
    pub artillery_formation_size_applications: u32,
    /// Honesty: DelayDeliveryMin/Max residual applications.
    #[serde(default)]
    pub artillery_delay_delivery_applications: u32,
    /// Honesty: WeaponErrorRadius residual applications.
    #[serde(default)]
    pub artillery_weapon_error_radius_applications: u32,
    /// Honesty: PreferredHeight residual applications.
    #[serde(default)]
    pub artillery_preferred_height_applications: u32,
    /// Honesty: shell detonation FireFX residual applications (per shell wave).
    #[serde(default)]
    pub artillery_fire_fx_applications: u32,
    /// Honesty: CruiseMissile residual pack armed at queue (Wave 56).
    #[serde(default)]
    pub cruise_residual_pack_armed: u32,
    /// Honesty: loft residual applications (SpecialSpeedTime / DistanceToTravel).
    #[serde(default)]
    pub cruise_loft_applications: u32,
    /// Honesty: HeightDie residual applications.
    #[serde(default)]
    pub cruise_height_die_applications: u32,
    /// Honesty: projectile object residual applications.
    #[serde(default)]
    pub cruise_projectile_applications: u32,
    /// Honesty: MOABDetonationWeapon residual applications.
    #[serde(default)]
    pub cruise_moab_weapon_applications: u32,
    /// Honesty: MOABFlameWeapon secondary residual applications.
    #[serde(default)]
    pub cruise_moab_flame_applications: u32,
    /// Honesty: MOAB FireFX residual applications.
    #[serde(default)]
    pub cruise_moab_fire_fx_applications: u32,
    /// Honesty: Nuke radiation residual pack applications (on parent strike).
    #[serde(default)]
    pub nuke_radiation_residual_pack_applications: u32,
    /// Honesty: Anthrax toxin residual pack applications (on parent strike).
    #[serde(default)]
    pub anthrax_toxin_residual_pack_applications: u32,
    /// C++ NeutronMissileSlowDeath lives on the flying missile. When OCL
    /// FireWeapon spawned a live (non-cruise) NeutronMissile, skip the
    /// registry instant blast + second SlowDeath at impact_frame.
    #[serde(default)]
    pub live_neutron_delivery: bool,
    /// C++ AttackNugget fires 9 ScudStormMissiles; FireWeaponWhenDead is
    /// ScudStormDamageWeapon. When leftover Attack scheduled live missiles,
    /// skip the registry blob warhead + FireOCL poison (one path only).
    #[serde(default)]
    pub live_scud_delivery: bool,
    /// C++ one CarpetBombWeapon per drop (HeightDie leftover). When flight
    /// leftover scheduled live bombs, skip registry line-wave 300/50 (one path).
    #[serde(default)]
    pub live_carpet_delivery: bool,
    /// C++ OCLSpecialPower A10Thunderbolt: CREATE_AT_EDGE_NEAR_SOURCE jets
    /// deliver per-missile warheads. When the flight leftover scheduled the
    /// jets, skip the registry impact wave entirely (one path only — the
    /// flight applies and credits its own damage).
    #[serde(default)]
    pub live_a10_delivery: bool,
    /// C++ one FireWeaponWhenDead on the falling AnthraxBomb. When flight
    /// leftover scheduled the cargo plane + bomb, skip registry 200/100 + toxin.
    #[serde(default)]
    pub live_anthrax_delivery: bool,
    /// C++ `initiateIntentToDoSpecialPower` `!COMMAND_FIRED_BY_SCRIPT`:
    /// human fire arms `m_manualTargetMode` so the beam holds the click
    /// instead of walking SwathOfDeath. Script-without-waypoint stays false.
    #[serde(default)]
    pub manual_beam_hold: bool,
    /// C++ `m_scriptedWaypointMode` — script waypoint fire drives the chain
    /// instead of SwathOfDeath. `manual_beam_hold` stays false.
    #[serde(default)]
    pub scripted_waypoint_mode: bool,
    /// C++ `m_nextDestWaypointID` after initiate first-link pick.
    #[serde(default)]
    pub next_dest_waypoint_id: u32,
    /// C++ `m_overrideTargetDestination` (first outgoing link, leftover Z-up
    /// converted to host Y-up).
    #[serde(default)]
    pub waypoint_override: Vec3,
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
    pub source_team: crate::game_logic::Team,
    pub hits: Vec<HostStrikeDamageHit>,
    /// Shell/bomb epicenters applied in this wave (presentation residual).
    pub epicenters: Vec<Vec3>,
    /// How many multi-strike shells/bombs this wave covers.
    pub wave_shell_count: u32,
    /// True when this wave finishes the strike (spawn fields / complete honesty).
    pub is_final_wave: bool,
}

/// Apply pre-fire intensity schedule residual onto a ParticleCannon strike.
///
/// Anchors ready-to-fire at `strike.impact_frame` (host beam spawn residual).
pub(crate) fn apply_particle_charge_status(
    strike: &mut HostSpecialPowerStrike,
    now: u32,
) -> Option<&'static str> {
    if strike.kind != HostSuperweaponKind::ParticleCannon {
        return None;
    }
    let next = particle_status_for_ready_countdown(now, strike.impact_frame);
    if next == strike.particle_status {
        return None;
    }
    strike.particle_status = next;
    strike.particle_intensity_transitions = strike.particle_intensity_transitions.saturating_add(1);
    if next.as_u8() > strike.particle_status_peak.as_u8() {
        strike.particle_status_peak = next;
    }
    match next {
        ParticleUplinkStatus::Charging => {
            strike.particle_charging_applications =
                strike.particle_charging_applications.saturating_add(1);
            // PoweringUpSoundLoop residual (STATUS_CHARGING).
            strike.particle_powerup_audio_applications =
                strike.particle_powerup_audio_applications.saturating_add(1);
            Some(PARTICLE_POWERUP_AUDIO)
        }
        ParticleUplinkStatus::Preparing => {
            strike.particle_preparing_applications =
                strike.particle_preparing_applications.saturating_add(1);
            strike.particle_model_unpacking_sets =
                strike.particle_model_unpacking_sets.saturating_add(1);
            // UnpackToIdleSoundLoop residual (STATUS_PREPARING).
            strike.particle_unpack_audio_applications =
                strike.particle_unpack_audio_applications.saturating_add(1);
            Some(PARTICLE_UNPACK_AUDIO)
        }
        ParticleUplinkStatus::AlmostReady => {
            strike.particle_almost_ready_applications =
                strike.particle_almost_ready_applications.saturating_add(1);
            strike.particle_model_deployed_sets =
                strike.particle_model_deployed_sets.saturating_add(1);
            None
        }
        ParticleUplinkStatus::ReadyToFire => {
            strike.particle_ready_applications =
                strike.particle_ready_applications.saturating_add(1);
            strike.particle_model_deployed_sets =
                strike.particle_model_deployed_sets.saturating_add(1);
            None
        }
        ParticleUplinkStatus::Packing => {
            strike.particle_model_packing_sets =
                strike.particle_model_packing_sets.saturating_add(1);
            None
        }
        _ => None,
    }
}
