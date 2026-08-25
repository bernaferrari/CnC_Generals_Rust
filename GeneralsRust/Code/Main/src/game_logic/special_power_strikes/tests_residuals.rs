use super::types::*;
use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::{ObjectId, Team};
use glam::Vec3;

#[test]
fn particle_uplink_damage_pulse_remnant_residual_honesty() {
    // Retail remnant weapon / lifetime residual constants.
    assert!((PARTICLE_REMNANT_DAMAGE_PER_TICK - 15.0).abs() < 0.01);
    assert!((PARTICLE_REMNANT_RADIUS - 10.0).abs() < 0.01);
    assert_eq!(PARTICLE_REMNANT_TICK_INTERVAL_FRAMES, 7);
    assert_eq!(PARTICLE_REMNANT_DURATION_FRAMES, 120);
    assert_eq!(
        PARTICLE_REMNANT_OBJECT_NAME,
        "ParticleUplinkCannonTrailRemnant"
    );
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
        (
            ObjectId(3),
            rem_pos + Vec3::new(50.0, 0.0, 0.0),
            Team::GLA,
            true,
        ),
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
        reg.remnant_fields().iter().all(|f| f.spawn_frame > spawn
            || f.is_expired(spawn + PARTICLE_REMNANT_DURATION_FRAMES)
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

    // Spawn frame: first grow step → tiny radius, still miss unit at dist 30.
    let early = reg.plan_due_beam_ticks(spawn, &objects);
    assert_eq!(early.len(), 1);
    assert!(early[0].hits.is_empty());
    let spawn_step = 1.0 / (PARTICLE_WIDTH_GROW_FRAMES as f32);
    let spawn_radius = PARTICLE_BEAM_RADIUS * spawn_step;
    assert!((early[0].width_scalar - spawn_step).abs() < 0.01);
    assert!((early[0].damage_radius - spawn_radius).abs() < 0.05);
    reg.record_beam_tick_complete(field_id, 0.0, 0, 0, spawn);

    // Advance to half grow (scalar 0.5 → radius 22.1) — still miss unit at 30.
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
    assert!((mid[0].damage_radius - 22.1).abs() < 0.1);
    assert!(
        mid[0].hits.is_empty(),
        "half-grow radius 22.1 must miss unit at dist 30"
    );
    reg.record_beam_tick_complete(field_id, 0.0, 0, 0, half);
    assert!((reg.beam_fields()[0].peak_width_scalar - 0.5).abs() < 0.01);

    // Full grow: radius 44.2 → hit unit at dist 30.
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
    reg.record_beam_tick_complete(field_id, PARTICLE_BEAM_DAMAGE_PER_PULSE, 1, 0, full);
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

    // Hold phase end (TotalFiringTime): full radius 44.2 → hit unit at dist 30.
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
    reg.record_beam_tick_complete(field_id, PARTICLE_BEAM_DAMAGE_PER_PULSE, 1, 0, decay_start);

    // Half-decay: scalar 0.5 → radius 22.1 → miss unit at dist 30.
    let half_decay = decay_start + PARTICLE_WIDTH_GROW_FRAMES / 2;
    if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
        f.next_tick_frame = half_decay;
        f.pulses_made = 0; // keep first swath epicenter for radius test
    }
    let mid = reg.plan_due_beam_ticks(half_decay, &objects);
    assert_eq!(mid.len(), 1);
    assert!((mid[0].width_scalar - 0.5).abs() < 0.01);
    assert!((mid[0].damage_radius - 22.1).abs() < 0.1);
    assert!(
        mid[0].hits.is_empty(),
        "half-decay radius 22.1 must miss unit at dist 30"
    );
    reg.record_beam_tick_complete(field_id, 0.0, 0, 0, half_decay);
    assert!(reg.beam_fields()[0].decay_samples > 0);
    assert!(reg.beam_fields()[0].trough_width_scalar < 0.51);
    assert!(reg.honesty_beam_width_decay_ok());

    // Sample-only path (no damage pulse) still tracks trough residual.
    let later = half_decay + 10;
    reg.sample_beam_width_honesty(later);
    assert!(reg.beam_fields()[0].trough_width_scalar < 0.4);
    assert!(
        (reg.beam_fields()[0].last_width_scalar - particle_width_scalar(spawn, later)).abs() < 0.01
    );

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
    assert!(
        reg.apply_due_beam_scorch_reveals(next.saturating_sub(1))
            .is_empty()
    );

    // Catch-up: jump past several scorch slots → multiple residual events.
    let late = spawn + PARTICLE_BEAM_DURATION_FRAMES;
    let caught = reg.apply_due_beam_scorch_reveals(late);
    assert!(
        caught.len() >= 5,
        "fractional scorch schedule catch-up, got {}",
        caught.len()
    );
    assert!(reg.beam_fields()[0].scorch_marks_made <= PARTICLE_TOTAL_SCORCH_MARKS);
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
        assert_eq!(
            f.outer_node_systems_created,
            PARTICLE_OUTER_EFFECT_NUM_BONES
        );
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
fn post_fire_object_override_applies_to_live_beam_and_orbit() {
    // Given: live PUC beam + Spectre orbit after fire.
    // When: the source object's override destination is applied on the strike tick.
    // Then: beam manual aim and orbit center follow that click.
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let source = ObjectId(7);
    let fire_pos = Vec3::new(40.0, 0.0, 10.0);
    let click = Vec3::new(220.0, 0.0, 180.0);
    let beam_id = reg.spawn_beam_field(source, Team::USA, fire_pos, 0, 1);
    let orbit_id = reg.spawn_orbit_field(source, Team::USA, fire_pos, 0, 2);
    assert!(reg.apply_source_override_destination(source, click, 1));
    let beam = reg
        .beam_fields()
        .iter()
        .find(|f| f.id == beam_id)
        .expect("beam");
    assert!(beam.manual_target_mode);
    assert!((beam.override_destination.x - click.x).abs() < 0.01);
    assert!((beam.override_destination.z - click.z).abs() < 0.01);
    let orbit = reg
        .orbit_fields()
        .iter()
        .find(|f| f.id == orbit_id)
        .expect("orbit");
    assert!((orbit.position.x - click.x).abs() < 0.01);
    assert!((orbit.position.z - click.z).abs() < 0.01);
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
    // Host combat uses the same peak ([`PARTICLE_BEAM_RADIUS`]).
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
        // Host combat radius is now retail 44.2 × 0.5 = 22.1.
        assert!((particle_beam_damage_radius(spawn, half) - 22.1).abs() < 0.1);
    }

    // Full hold: draw 26, laser 13, retail/combat damage 44.2.
    let hold = spawn + PARTICLE_WIDTH_GROW_FRAMES;
    reg.sample_beam_width_honesty(hold);
    {
        let f = &reg.beam_fields()[0];
        assert!((f.peak_outer_beam_draw_width - 26.0).abs() < 0.1);
        assert!((f.peak_retail_laser_radius - 13.0).abs() < 0.1);
        assert!((f.peak_retail_damage_radius - 44.2).abs() < 0.1);
        assert!((f.last_outer_beam_draw_width - 26.0).abs() < 0.1);
        assert!((particle_beam_damage_radius(spawn, hold) - 44.2).abs() < 0.1);
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

/// C++ PartitionFilterAlive only — same-team units in the beam take 35/pulse.
#[test]
fn particle_uplink_beam_pulses_hit_same_team() {
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
    let hold = spawn + PARTICLE_WIDTH_GROW_FRAMES;
    if let Some(f) = reg.beam_fields.iter_mut().find(|f| f.id == field_id) {
        f.next_tick_frame = hold;
        f.pulses_made = 0;
    }
    let epic0 = particle_swath_epicenter(click, 0);
    let objects = vec![
        (ObjectId(1), Vec3::new(500.0, 0.0, 0.0), Team::China, true),
        (ObjectId(2), epic0, Team::China, true),
        (ObjectId(3), epic0, Team::GLA, true),
        (ObjectId(4), epic0, Team::Neutral, true),
    ];
    let plans = reg.plan_due_beam_ticks(hold, &objects);
    assert_eq!(plans.len(), 1);
    assert!((plans[0].damage_radius - 44.2).abs() < 0.1);
    let mut hits: Vec<_> = plans[0].hits.iter().map(|h| h.target_id).collect();
    hits.sort_by_key(|id| id.0);
    assert_eq!(
        hits,
        vec![ObjectId(2), ObjectId(3), ObjectId(4)],
        "beam must hit same-team, enemy, and neutral; launcher still excluded"
    );
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
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
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
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(110.0, 0.0, 90.0)]);
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
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
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
        (particle_orbital_soft_edge_alpha(0) - PARTICLE_ORBITAL_LASER_INNER_COLOR.3).abs() < 0.01
    );
    assert!(
        (particle_orbital_soft_edge_alpha(11) - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs() < 0.01
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
        assert!(
            (f.last_outer_node_bone_position.x - (origin.x + PARTICLE_OUTER_NODE_RING_RADIUS))
                .abs()
                < 0.01
        );
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
    let (pos, traveled, dist_to, phase) = scud_missile_ballistic_sample(launch, target, 120);
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
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
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

#[test]
fn particle_uplink_connector_soft_edge_residual_honesty() {
    assert_eq!(PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS, 5);
    assert!((particle_connector_intense_soft_edge_scale(0) - 0.0).abs() < 0.01);
    assert!((particle_connector_intense_soft_edge_scale(4) - 1.0).abs() < 0.01);
    assert!((particle_connector_intense_soft_edge_width(0) - 0.6).abs() < 0.01);
    assert!((particle_connector_intense_soft_edge_width(4) - 2.0).abs() < 0.01);
    let (r, _g, b, _) = particle_connector_intense_soft_edge_color(4);
    assert!((r - 0.0).abs() < 0.01 && (b - 1.0).abs() < 0.01);
    assert_eq!(PARTICLE_CONNECTOR_LASER_TEXTURE, "EXLaser.tga");
    assert!((PARTICLE_CONNECTOR_MEDIUM_INNER_BEAM_WIDTH - 0.4).abs() < 0.01);

    let origin = Vec3::new(5.0, 0.0, 5.0);
    let (start, end) = particle_connector_laser_segment(origin, 0);
    assert!((start.y - PARTICLE_OUTER_NODE_RING_HEIGHT).abs() < 0.01);
    assert!((end.x - origin.x).abs() < 0.01);
    assert!((end.y - PARTICLE_OUTER_NODE_RING_HEIGHT).abs() < 0.01);

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
        assert_eq!(f.connector_soft_edge_armed, 1);
        assert_eq!(f.connector_laser_segments_created, 5);
        assert!((f.peak_connector_soft_edge_outer_width - 2.0).abs() < 0.01);
        assert!((f.last_connector_segment_end.x - origin.x).abs() < 0.01);
    }
    assert!(reg.honesty_beam_connector_soft_edge_ok());
    assert!(reg.honesty_beam_outer_node_bone_layout_ok());
}

#[test]
fn scud_thrust_wobble_residual_honesty() {
    assert!((SCUD_STORM_MISSILE_THRUST_ROLL - 0.06).abs() < 0.001);
    assert!((SCUD_STORM_MISSILE_THRUST_WOBBLE_RATE - 0.008).abs() < 0.001);
    assert!((SCUD_STORM_MISSILE_THRUST_MIN_WOBBLE + 0.040).abs() < 0.001);
    assert!((SCUD_STORM_MISSILE_THRUST_MAX_WOBBLE - 0.040).abs() < 0.001);
    assert!(SCUD_STORM_MISSILE_CLOSE_ENOUGH_DIST_3D);
    let w0 = scud_missile_thrust_wobble(0);
    assert!(w0.abs() <= 0.040 + 0.001);
    let w100 = scud_missile_thrust_wobble(100);
    assert!(w100.abs() <= 0.040 + 0.001);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_thrust_wobble_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_thrust_wobble_applications, 1);
        assert!(s.scud_peak_abs_thrust_wobble > 0.0);
    }
    assert!(reg.honesty_scud_thrust_wobble_ok());
    assert!(reg.honesty_scud_ballistic_flight_ok());
}

#[test]
fn particle_uplink_medium_connector_soft_edge_residual_honesty() {
    assert_eq!(PARTICLE_CONNECTOR_MEDIUM_NUM_BEAMS, 4);
    assert!((particle_connector_medium_soft_edge_scale(0) - 0.0).abs() < 0.01);
    assert!((particle_connector_medium_soft_edge_scale(3) - 1.0).abs() < 0.01);
    assert!((particle_connector_medium_soft_edge_width(0) - 0.4).abs() < 0.01);
    assert!((particle_connector_medium_soft_edge_width(3) - 1.2).abs() < 0.01);
    let (r, _g, b, _) = particle_connector_medium_soft_edge_color(3);
    assert!((r - 0.0).abs() < 0.01 && (b - 1.0).abs() < 0.01);

    let origin = Vec3::new(5.0, 0.0, 5.0);
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        origin,
        0,
    );
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, origin, 120, strike_id);
    assert!(!reg.honesty_beam_connector_medium_soft_edge_ok());
    // Advance into POSTFIRE (after TotalFiringTime) for Medium connector residual.
    let postfire_frame = 120 + PARTICLE_BEAM_DURATION_FRAMES;
    reg.advance_particle_intensity_schedule(postfire_frame);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert_eq!(f.status, ParticleUplinkStatus::Postfire);
        assert_eq!(f.connector_intensity, ParticleIntensity::Medium);
        assert!(f.medium_connector_soft_edge_armed >= 1);
        assert!((f.peak_medium_connector_soft_edge_outer_width - 1.2).abs() < 0.01);
    }
    assert!(reg.honesty_beam_connector_medium_soft_edge_ok());
    assert!(reg.honesty_beam_connector_soft_edge_ok());
}

#[test]
fn particle_uplink_orbital_vision_shroud_residual_honesty() {
    assert!((PARTICLE_ORBITAL_LASER_VISION_RANGE - 100.0).abs() < 0.01);
    assert!((PARTICLE_ORBITAL_LASER_SHROUD_CLEARING_RANGE - 120.0).abs() < 0.01);
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    assert!(!reg.honesty_beam_vision_shroud_ok());
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, Vec3::ZERO, 10, strike_id);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert_eq!(f.orbital_vision_shroud_armed, 1);
        assert!((f.last_orbital_vision_range - 100.0).abs() < 0.01);
        assert!((f.last_orbital_shroud_clearing_range - 120.0).abs() < 0.01);
    }
    assert!(reg.honesty_beam_vision_shroud_ok());
}

#[test]
fn particle_uplink_soft_edge_premul_residual_honesty() {
    let ia = PARTICLE_ORBITAL_LASER_INNER_COLOR.3;
    let (r0, _, _, _) = particle_orbital_soft_edge_color_premul(0);
    let (r11, _, _, a11) = particle_orbital_soft_edge_color_premul(11);
    assert!((r0 - 1.0).abs() < 0.01);
    assert!((r11 - (1.0 - ia)).abs() < 0.01);
    assert!((a11 - PARTICLE_ORBITAL_LASER_OUTER_COLOR.3).abs() < 0.01);
    // Premul outer red is less than linear outer red (0.0) wait: linear outer red is 0;
    // premul outer red = 1 + 1*(0-1)*ia = 1-ia > 0 for ia < 1.
    let (lin_r, _, _, _) = particle_orbital_soft_edge_color(11);
    assert!((lin_r - 0.0).abs() < 0.01);
    assert!(r11 > lin_r);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, Vec3::ZERO, 0, strike_id);
    assert!(!reg.honesty_beam_soft_edge_premul_ok());
    reg.sample_beam_width_honesty(PARTICLE_WIDTH_GROW_FRAMES);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert!(f.soft_edge_premul_samples >= 1);
        assert!((f.last_soft_edge_premul_outer_r - (1.0 - ia)).abs() < 0.01);
    }
    assert!(reg.honesty_beam_soft_edge_premul_ok());
}

#[test]
fn scud_object_params_residual_honesty() {
    assert!((SCUD_STORM_MISSILE_VISION_RANGE - 300.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_SHROUD_CLEARING_RANGE - 0.0).abs() < 0.01);
    assert_eq!(SCUD_STORM_MISSILE_KIND_OF, "PROJECTILE");
    assert_eq!(SCUD_STORM_MISSILE_ARMOR, "ProjectileArmor");
    assert_eq!(SCUD_STORM_MISSILE_TRANSPORT_SLOT_COUNT, 10);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_object_params_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_object_params_applications, 1);
    }
    assert!(reg.honesty_scud_object_params_ok());
    assert!(reg.honesty_scud_geometry_ok());
}

#[test]
fn spectre_howitzer_shell_object_params_residual_honesty() {
    assert_eq!(SPECTRE_HOWITZER_SHELL_KIND_OF, "PROJECTILE");
    assert!((SPECTRE_HOWITZER_SHELL_VISION_RANGE - 0.0).abs() < 0.01);
    assert_eq!(SPECTRE_HOWITZER_SHELL_ARMOR, "ProjectileArmor");

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
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_shell_object_params_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_object_params_applications, 1);
    }
    assert!(reg.honesty_howitzer_shell_object_params_ok());
}

#[test]
fn scud_geometry_residual_honesty() {
    assert_eq!(SCUD_STORM_MISSILE_GEOMETRY, "Cylinder");
    assert!(SCUD_STORM_MISSILE_GEOMETRY_IS_SMALL);
    assert!((SCUD_STORM_MISSILE_GEOMETRY_RADIUS - 7.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_GEOMETRY_HEIGHT - 30.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_MASS - 500.0).abs() < 0.01);
    assert!((SCUD_STORM_MISSILE_MAX_HEALTH - 10000.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let id = reg.queue(
        HostSuperweaponKind::ScudStorm,
        ObjectId(1),
        Team::GLA,
        Vec3::new(100.0, 0.0, 100.0),
        0,
    );
    assert!(!reg.honesty_scud_geometry_ok());
    reg.record_impact_wave(id, 0.0, 0, 0, 1, false, &[Vec3::new(100.0, 0.0, 100.0)]);
    {
        let s = reg.get(id).unwrap();
        assert_eq!(s.scud_geometry_applications, 1);
    }
    assert!(reg.honesty_scud_geometry_ok());
    assert!(reg.honesty_scud_ballistic_flight_ok());
}

#[test]
fn spectre_howitzer_shell_lasered_ocl_residual_honesty() {
    assert_eq!(
        SPECTRE_HOWITZER_SHELL_DEATH_LASERED_OCL,
        "OCL_GenericMissileDisintegrate"
    );
    assert!(SPECTRE_HOWITZER_SHELL_DEATH_LASERED_FX.contains("Disintegrate"));

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
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_death_lasered_applications, 1);
        assert_eq!(f.howitzer_shell_death_lasered_ocl_applications, 1);
    }
    assert!(reg.honesty_howitzer_shell_dumb_projectile_ok());
    assert!(reg.honesty_howitzer_shell_ok());
}

#[test]
fn particle_uplink_laser_update_client_residual_honesty() {
    assert!((PARTICLE_LASER_ORBIT_ALTITUDE - 500.0).abs() < 0.01);
    assert_eq!(PARTICLE_WIDTH_GROW_FRAMES, 60);
    assert!((laser_update_width_scalar_widen(0, 60) - 0.0).abs() < 0.01);
    assert!((laser_update_width_scalar_widen(30, 60) - 0.5).abs() < 0.01);
    assert!((laser_update_width_scalar_widen(60, 60) - 1.0).abs() < 0.01);
    assert!((laser_update_width_scalar_decay(0, 60) - 1.0).abs() < 0.01);
    assert!((laser_update_width_scalar_decay(60, 60) - 0.0).abs() < 0.01);
    assert!((laser_update_current_radius(1.0) - 13.0).abs() < 0.01);

    let target = Vec3::new(10.0, 0.0, 20.0);
    let (g_start, g_end) = particle_ground_to_orbit_laser_segment(target);
    assert!((g_end.y - (target.y + 500.0)).abs() < 0.01);
    assert!((g_start.x - target.x).abs() < 0.01);
    let (o_start, o_end) = particle_orbit_to_target_laser_segment(target);
    assert!((o_start.y - 500.0).abs() < 0.01);
    assert!((o_end - target).length() < 0.01);
    let mid = laser_update_drawable_midpoint(o_start, o_end);
    assert!((mid.y - 250.0).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        target,
        0,
    );
    assert!(!reg.honesty_beam_laser_update_ok());
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, target, 120, strike_id);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert_eq!(f.laser_update_init_applications, 2);
        assert!(f.laser_update_dirty);
        assert_eq!(f.laser_update_growth_frames, 60);
        assert!(f.laser_update_widening);
        assert!(!f.laser_update_decaying);
        assert!((f.last_laser_update_start.y - 500.0).abs() < 0.01);
        assert!((f.last_laser_update_end - target).length() < 0.01);
        assert!((f.last_laser_update_drawable_mid.y - 250.0).abs() < 0.01);
    }
    assert!(reg.honesty_beam_laser_update_ok());

    // Mid-grow sample residual.
    reg.sample_beam_width_honesty(120 + 30);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert!((f.laser_update_current_width_scalar - 0.5).abs() < 0.01);
        assert!((f.last_laser_update_radius - 6.5).abs() < 0.01);
        assert!(f.laser_update_widening);
        assert!(!f.laser_update_decaying);
    }

    // POSTFIRE decay residual.
    let postfire_frame = 120 + PARTICLE_BEAM_DURATION_FRAMES;
    reg.advance_particle_intensity_schedule(postfire_frame);
    reg.sample_beam_width_honesty(postfire_frame);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert!(f.laser_update_decaying);
        assert!(!f.laser_update_widening);
        assert!(f.laser_update_dirty);
    }
    assert!(reg.honesty_beam_laser_update_ok());
    assert!(reg.honesty_beam_vision_shroud_ok());
}

#[test]
fn spectre_howitzer_shell_loft_flight_residual_honesty() {
    let spawn = Vec3::new(0.0, 80.0, 0.0);
    let target = Vec3::new(10.0, 0.0, 0.0);
    let (pos_early, moving_early, die_early) = howitzer_shell_loft_sample(spawn, target, 10);
    assert!(!die_early, "pad-safe: no HeightDie before InitialDelay");
    assert!(!moving_early || pos_early.y >= SPECTRE_HOWITZER_HEIGHT_DIE_TARGET_HEIGHT);
    let (pos_late, _moving_late, die_late) = howitzer_shell_loft_sample(spawn, target, 45);
    assert!(die_late, "HeightDie after InitialDelay + sink");
    assert!((pos_late.y - 0.0).abs() < 0.01);

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
    let spawn_f = reg.orbit_fields()[0].spawn_frame;
    assert!(!reg.honesty_howitzer_shell_loft_flight_ok());
    reg.record_orbit_tick_complete(field_id, 80.0, 1, 0, spawn_f);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.howitzer_shell_loft_flight_applications, 1);
        assert!(f.howitzer_shell_loft_height_die_applications >= 1);
    }
    assert!(reg.honesty_howitzer_shell_loft_flight_ok());
    assert!(reg.honesty_howitzer_shell_model_draw_ok());
}

#[test]
fn particle_uplink_connector_soft_edge_premul_residual_honesty() {
    let ia = PARTICLE_CONNECTOR_INNER_COLOR.3;
    let (r0, _, _, _) = particle_connector_intense_soft_edge_color_premul(0);
    let (r4, _, _, a4) = particle_connector_intense_soft_edge_color_premul(4);
    assert!((r0 - 1.0).abs() < 0.01);
    assert!((r4 - (1.0 - ia)).abs() < 0.01);
    assert!((a4 - PARTICLE_CONNECTOR_OUTER_COLOR.3).abs() < 0.01);
    let (lin_r, _, _, _) = particle_connector_intense_soft_edge_color(4);
    assert!((lin_r - 0.0).abs() < 0.01);
    assert!(r4 > lin_r);
    // Medium premul residual uses same formula.
    let (mr3, _, _, _) = particle_connector_medium_soft_edge_color_premul(3);
    assert!((mr3 - (1.0 - ia)).abs() < 0.01);

    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let strike_id = reg.queue(
        HostSuperweaponKind::ParticleCannon,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, Vec3::ZERO, 0, strike_id);
    assert!(!reg.honesty_beam_connector_soft_edge_premul_ok());
    reg.sample_beam_width_honesty(PARTICLE_WIDTH_GROW_FRAMES);
    {
        let f = reg.beam_fields().iter().find(|b| b.id == field_id).unwrap();
        assert!(f.connector_soft_edge_premul_samples >= 1);
        assert!((f.last_connector_soft_edge_premul_outer_r - (1.0 - ia)).abs() < 0.01);
    }
    assert!(reg.honesty_beam_connector_soft_edge_premul_ok());
    assert!(reg.honesty_beam_soft_edge_premul_ok());
}
