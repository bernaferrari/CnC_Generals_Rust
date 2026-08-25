use super::types::*;
use super::*;
use crate::command_system::SpecialPowerType;
use crate::game_logic::{ObjectId, Team};
use glam::Vec3;

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
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::NapalmStrike),
        Some(HostSuperweaponKind::NapalmStrike)
    );
    assert_ne!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::NapalmStrike),
        Some(HostSuperweaponKind::DaisyCutter)
    );
    assert!(
        (HostSuperweaponKind::NapalmStrike.max_damage() - NAPALM_STRIKE_PRIMARY_DAMAGE).abs() < 0.1
    );
    assert!(
        (HostSuperweaponKind::NapalmStrike.max_damage() - DAISY_CUTTER_PRIMARY_DAMAGE).abs() > 1.0
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
    assert_eq!(kind.impact_delay_frames(), PARTICLE_BEAM_TRAVEL_FRAMES);
    assert_eq!(kind.impact_delay_frames(), 75);
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
    assert!((PARTICLE_BEAM_RADIUS - 44.2).abs() < 0.01);
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
    // Grow phase: spawn frame gets first grow step so damage pulse is non-zero.
    let spawn_step = 1.0 / (PARTICLE_WIDTH_GROW_FRAMES as f32);
    assert!((particle_width_scalar(100, 100) - spawn_step).abs() < 0.01);
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
    assert!((particle_beam_damage_radius(100, half_decay) - 22.1).abs() < 0.1);
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
    assert!(
        o_mid.x.abs() < 1.0,
        "mid swath along-axis near 0, got {}",
        o_mid.x
    );
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
    assert_eq!(
        reg.get(id).unwrap().impact_frame,
        PARTICLE_BEAM_TRAVEL_FRAMES
    );
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
    // WidthGrow residual: first grow step at spawn so pulse has non-zero radius
    // but still below dist-to-ObjectId(3) (~30).
    let beam_plans = reg.plan_due_beam_ticks(120, &objects);
    assert_eq!(beam_plans.len(), 1);
    assert!(
        (beam_plans[0].position.x - swath0.x).abs() < 0.1,
        "first pulse must use swath epicenter"
    );
    let spawn_step = 1.0 / (PARTICLE_WIDTH_GROW_FRAMES as f32);
    let spawn_radius = PARTICLE_BEAM_RADIUS * spawn_step;
    assert!((beam_plans[0].width_scalar - spawn_step).abs() < 0.01);
    assert!((beam_plans[0].damage_radius - spawn_radius).abs() < 0.05);
    assert_eq!(beam_plans[0].hits.len(), 1); // epicenter only under tiny radius
    assert_eq!(beam_plans[0].hits[0].target_id, ObjectId(2));
    assert!(
        !beam_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3))
    );
    assert!(
        !beam_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(4))
    );
    assert!(
        !beam_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(5))
    );

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
    // WidthGrow residual: first pulse at spawn records first grow step peak.
    let spawn_step = 1.0 / (PARTICLE_WIDTH_GROW_FRAMES as f32);
    assert!(
        (reg.beam_fields()[0].peak_width_scalar - spawn_step).abs() < 0.01,
        "peak_width_scalar {}",
        reg.beam_fields()[0].peak_width_scalar
    );
    // Fractional nextFactor: pulses_made=1 → factor 1/40 * 105 = 2.625 → floor 2.
    let expected_next = particle_next_pulse_frame(120, 1).max(121);
    assert_eq!(reg.beam_fields()[0].next_tick_frame, expected_next);
    assert_eq!(reg.beam_fields()[0].pulses_made, 1);

    // Not due again until scheduled frame.
    assert!(
        reg.plan_due_beam_ticks(expected_next.saturating_sub(1), &objects)
            .is_empty()
    );
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
fn puc_disabled_aborts_live_beam_matches_cpp_mask() {
    assert!(puc_disabled_aborts_live_beam(true, false, false, false));
    assert!(puc_disabled_aborts_live_beam(false, true, false, false));
    assert!(puc_disabled_aborts_live_beam(false, false, true, false));
    assert!(puc_disabled_aborts_live_beam(false, false, false, true));
    assert!(!puc_disabled_aborts_live_beam(false, false, false, false));
}

#[test]
fn particle_cannon_owner_disable_starts_decay_and_stops_pulses() {
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let target = Vec3::ZERO;
    let spawn = 10;
    let field_id = reg.spawn_beam_field(ObjectId(1), Team::USA, target, spawn, 1);
    let objects = vec![
        (ObjectId(1), Vec3::new(-400.0, 0.0, 0.0), Team::USA, true),
        (
            ObjectId(2),
            particle_swath_epicenter(target, 0),
            Team::GLA,
            true,
        ),
    ];
    let first = reg.plan_due_beam_ticks(spawn, &objects);
    assert_eq!(first.len(), 1);
    assert!(!first[0].hits.is_empty());
    reg.record_beam_tick_complete(field_id, PARTICLE_BEAM_DAMAGE_PER_PULSE, 1, 0, spawn);

    let abort_frame = spawn + 3;
    reg.abort_beam_fields_on_owner_disable(abort_frame, |id| id == ObjectId(1));
    let field = &reg.beam_fields()[0];
    assert_eq!(field.start_decay_frame, abort_frame);
    assert_eq!(
        field.expires_frame,
        abort_frame.saturating_add(PARTICLE_WIDTH_GROW_FRAMES)
    );
    assert!(reg.plan_due_beam_ticks(abort_frame, &objects).is_empty());
    assert!(
        reg.plan_due_beam_ticks(abort_frame + 1, &objects)
            .is_empty()
    );
    assert_eq!(reg.beam_fields()[0].pulses_made, 1);
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
    assert_eq!(
        carpet_bomb_impact_frame(0, 0),
        CARPET_BOMB_IMPACT_DELAY_FRAMES
    );
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
    // Queue used Team::China → CarpetBombFactionTier::China (10 bombs).
    let points = carpet_bomb_points_for_tier(target, CarpetBombFactionTier::China);
    let first = points[0];
    let center = points[points.len() / 2];
    let outer = *points.last().expect("outer");
    let objects = vec![
        (ObjectId(1), Vec3::ZERO, Team::China, true),
        (ObjectId(2), center, Team::USA, true), // center bomb (with variance)
        (ObjectId(3), outer, Team::USA, true),  // outer bomb (with variance)
        (ObjectId(4), Vec3::new(0.0, 0.0, 500.0), Team::USA, true), // far off-line
        (ObjectId(5), center, Team::China, true), // friendly
        (ObjectId(6), first, Team::USA, true),  // first bomb DropDelay residual
    ];

    // Before first bomb: no damage plan.
    assert!(
        reg.plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty()
    );

    // First DropDelay wave: only bomb 0 due — not complete.
    let first_wave = reg.plan_due_impacts(CARPET_BOMB_IMPACT_DELAY_FRAMES, &objects);
    assert_eq!(first_wave.len(), 1);
    assert_eq!(first_wave[0].wave_shell_count, 1);
    assert!(!first_wave[0].is_final_wave);
    assert!(
        first_wave[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(6) && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1)
    );
    // Center (index 7) and outer (index 14) not yet due.
    assert!(
        !first_wave[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2))
    );
    assert!(
        !first_wave[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3))
    );
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

    // Jump to last China-tier bomb frame: remaining bombs (incl. center + outer) apply.
    let china_count = CarpetBombFactionTier::China.bomb_count();
    let last = carpet_bomb_impact_frame_for_tier(
        0,
        china_count.saturating_sub(1),
        CarpetBombFactionTier::China,
    );
    let plans = reg.plan_due_impacts(last, &objects);
    assert_eq!(plans.len(), 1);
    assert!(plans[0].is_final_wave);
    // Remaining after first-wave apply: china_count - 1.
    assert_eq!(plans[0].wave_shell_count, china_count.saturating_sub(1));
    // Center + outer-bomb enemies + friendly (ALLIES residual); far excluded.
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2) && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1)
    );
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3) && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1)
    );
    assert!(!plans[0].hits.iter().any(|h| h.target_id == ObjectId(4)));
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(5) && (h.damage - CARPET_BOMB_DAMAGE).abs() < 0.1)
    );

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
    assert_eq!(reg.get(id).unwrap().multi_strike_applied, china_count);
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
    assert_eq!(
        delay_delivery_frames(0, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES),
        0
    );
    for i in 1..12 {
        let d = delay_delivery_frames(i, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
        assert!(d <= ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
    }
    // WeaponErrorRadius residual: index 0 spot-on; others within error radius.
    assert_eq!(
        weapon_error_radius_offset(0, ARTILLERY_BARRAGE_ERROR_RADIUS),
        Vec3::ZERO
    );
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
    assert!(
        any_scatter,
        "WeaponErrorRadius residual must scatter non-lead shells"
    );
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
    assert!(
        reg.plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty()
    );

    // First wave: lead shell (DelayDelivery 0) — center hit; not necessarily final.
    let first = reg.plan_due_impacts(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, &objects);
    assert_eq!(first.len(), 1);
    assert!(
        first[0].hits.iter().any(
            |h| h.target_id == ObjectId(2) && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1
        )
    );
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
        assert!(
            plans[0]
                .hits
                .iter()
                .any(|h| h.target_id == ObjectId(3)
                    && (h.damage - ARTILLERY_BARRAGE_DAMAGE).abs() < 0.1)
                || first[0].hits.iter().any(|h| h.target_id == ObjectId(3))
        );
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
    assert!(
        any_positive,
        "DelayDelivery residual must stagger some shells"
    );
    // Shell impact frames: base + delay.
    assert_eq!(
        artillery_shell_impact_frame(10, 0),
        10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES
    );
    assert!(artillery_shell_impact_frame(10, 5) >= 10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
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
    assert!(
        reg.plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES - 1, &objects)
            .is_empty()
    );

    let plans = reg.plan_due_impacts(CRUISE_MISSILE_IMPACT_DELAY_FRAMES, &objects);
    assert_eq!(plans.len(), 1);
    // Epicenter + near enemy + friendly (ALLIES residual); far excluded.
    // Epicenter damage = MOAB primary + MOABFlame secondary residual.
    let expected_epicenter = CRUISE_MISSILE_DAMAGE + MOAB_FLAME_DAMAGE;
    assert_eq!(plans[0].hits.len(), 3);
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2) && (h.damage - expected_epicenter).abs() < 0.1)
    );
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3) && h.damage > MOAB_FLAME_DAMAGE)
    );
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(5) && (h.damage - expected_epicenter).abs() < 0.1)
    );
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
    let gattling_only =
        reg.plan_due_orbit_ticks(90 + SPECTRE_GATTLING_TICK_INTERVAL_FRAMES, &objects);
    assert_eq!(gattling_only.len(), 1);
    assert_eq!(gattling_only[0].hits.len(), 1);
    assert!((gattling_only[0].hits[0].damage - SPECTRE_GATTLING_DAMAGE).abs() < 0.01);
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
fn spectre_gattling_skips_target_under_gunship() {
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let source = ObjectId(1);
    let pos = Vec3::new(100.0, 0.0, 100.0);
    let _ = reg.spawn_orbit_field(source, Team::USA, pos, 90, 1);
    // Ship passing over the reticle — leftover isFairDistanceFromShip is false.
    reg.orbit_fields_mut()[0].gunship_position = Some(pos);
    let objects = vec![
        (source, Vec3::ZERO, Team::USA, true),
        (ObjectId(2), pos, Team::GLA, true),
    ];
    let plans = reg.plan_due_orbit_ticks(90, &objects);
    assert_eq!(plans.len(), 1);
    for hit in &plans[0].hits {
        assert!(
            (hit.damage - SPECTRE_GATTLING_DAMAGE).abs() > 0.01,
            "gattling must not acquire a target under the ship"
        );
        assert!(
            (hit.damage - (SPECTRE_ORBIT_DAMAGE_PER_TICK + SPECTRE_GATTLING_DAMAGE)).abs() > 0.01,
            "gattling must not stack onto howitzer under the ship"
        );
    }
}

#[test]
fn spectre_gattling_fail_closes_without_gunship_position() {
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    let source = ObjectId(1);
    let pos = Vec3::new(100.0, 0.0, 100.0);
    let _ = reg.spawn_orbit_field(source, Team::USA, pos, 90, 1);
    reg.orbit_fields_mut()[0].gunship_position = None;
    let objects = vec![
        (source, Vec3::ZERO, Team::USA, true),
        (ObjectId(2), pos, Team::GLA, true),
    ];
    let plans = reg.plan_due_orbit_ticks(90, &objects);
    assert_eq!(plans.len(), 1);
    for hit in &plans[0].hits {
        assert!(
            (hit.damage - SPECTRE_GATTLING_DAMAGE).abs() > 0.01,
            "missing ship position must fail-close acquire"
        );
    }
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
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2) && (h.damage - 200.0).abs() < 0.1)
    );
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(3) && (h.damage - 200.0).abs() < 0.1)
    );

    reg.record_impact_complete(id, 400.0, 2, 0);
    assert!(reg.honesty_complete_ok(HostSuperweaponKind::AnthraxBomb));
    assert!(reg.honesty_toxin_ok());
    assert!(reg.honesty_host_path_ok(HostSuperweaponKind::AnthraxBomb));
    assert_eq!(reg.toxin_fields().len(), 1);
    assert_eq!(reg.toxin_fields()[0].parent_strike_id, id);
    assert!(reg.radiation_fields().is_empty());

    // Toxin tick hits all teams in radius (retail ALLIES ENEMIES NEUTRALS).
    let tox_objects: Vec<_> = objects
        .iter()
        .copied()
        .map(|(id, pos, team, alive)| (id, pos, team, alive, false))
        .chain(std::iter::once((
            ObjectId(99),
            Vec3::new(100.0, 0.0, 100.0),
            Team::USA,
            true,
            true,
        )))
        .collect();
    let tox_plans = reg.plan_due_toxin_ticks(90, &tox_objects);
    assert_eq!(tox_plans.len(), 1);
    // source (1) excluded; epicenter USA (2) + GLA friendly (3) hit; far (4) not.
    // Airborne (99) skipped — C++ NOT_AIRBORNE / WEAPON_DOESNT_AFFECT_AIRBORNE.
    assert_eq!(tox_plans[0].hits.len(), 2);
    assert!(
        tox_plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - ANTHRAX_TOXIN_DAMAGE_PER_TICK).abs() < 0.01)
    );
    assert!(tox_plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));
    assert!(
        !tox_plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(99))
    );
    assert_eq!(
        tox_plans[0].death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::PoisonedBeta
    );

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
    // Instant blast suppressed — NeutronMissileSlowDeath multi-blast residual
    // applies Blast6MaxDamage on schedule (max_damage honesty stays 3500).
    assert!(
        plans[0].hits.iter().all(|h| h.damage == 0.0),
        "nuclear instant impact damage deferred to multi-blast residual"
    );
    assert!((HostSuperweaponKind::NuclearMissile.max_damage() - 3500.0).abs() < 0.1);

    reg.record_impact_complete(id, 0.0, 0, 0);
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
    assert!(
        rad_plans[0].hits.iter().any(|h| h.target_id == ObjectId(2)
            && (h.damage - NUKE_RADIATION_DAMAGE_PER_TICK).abs() < 0.01)
    );
    assert!(rad_plans[0].hits.iter().any(|h| h.target_id == ObjectId(3)));

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
    assert!((HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 100.0) - 2000.0).abs() < 0.1);
    let mid = HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 135.0);
    assert!(
        (mid - 1000.0).abs() < 1.0,
        "mid falloff expected ~1000, got {mid}"
    );
    assert_eq!(
        HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 170.0),
        0.0
    );
}

#[test]
fn friendly_fire_allies_residual_and_source_excluded() {
    // A10 no longer applies a delayed circle blast — OCL jets do the damage.
    let mut a10 = HostSpecialPowerStrikeRegistry::new();
    a10.queue(
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
    let a10_plans = a10.plan_due_impacts(60, &objects);
    assert!(
        a10_plans[0].hits.is_empty(),
        "A10 host blob must not apply circle damage"
    );

    // DaisyCutter retail RadiusDamageAffects includes ALLIES — friendly is hit.
    let mut reg = HostSpecialPowerStrikeRegistry::new();
    reg.queue(
        HostSuperweaponKind::DaisyCutter,
        ObjectId(1),
        Team::USA,
        Vec3::ZERO,
        0,
    );
    let plans = reg.plan_due_impacts(90, &objects);
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
        (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 0.0) - SCUD_STORM_PRIMARY_DAMAGE)
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
    assert!(HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 201.0).abs() < 0.1);

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
    assert_eq!(
        scud_missile_impact_frame(0, 0),
        SCUD_STORM_PRE_ATTACK_FRAMES
    );
    assert!(scud_missile_impact_frame(0, 1) > scud_missile_impact_frame(0, 0));
    assert!(scud_missile_impact_frame(0, 8) > scud_missile_impact_frame(0, 1));
    let last = multi_strike_last_impact_frame(kind, 0, ArtilleryBarrageScienceTier::Level1);
    assert_eq!(
        last,
        scud_missile_impact_frame(0, SCUD_STORM_MISSILE_COUNT - 1)
    );

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
    assert!(
        reg.plan_due_impacts(SCUD_STORM_PRE_ATTACK_FRAMES - 1, &objects)
            .is_empty()
    );

    // First missile wave.
    let plans = reg.plan_due_impacts(SCUD_STORM_PRE_ATTACK_FRAMES, &objects);
    assert_eq!(plans.len(), 1);
    assert!(!plans[0].is_final_wave);
    assert!(plans[0].wave_shell_count >= 1);
    assert!(
        plans[0]
            .hits
            .iter()
            .any(|h| h.target_id == ObjectId(2)
                && (h.damage - SCUD_STORM_PRIMARY_DAMAGE).abs() < 0.1)
    );
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
    assert_eq!(
        field.tick_interval_frames,
        SCUD_STORM_POISON_TICK_INTERVAL_FRAMES
    );
    assert_eq!(
        field.expires_frame,
        field.spawn_frame + SCUD_STORM_POISON_DURATION_FRAMES
    );

    // Poison tick uses LargePoison residual damage (one plan per field).
    let tox_objects: Vec<_> = objects
        .iter()
        .copied()
        .map(|(id, pos, team, alive)| (id, pos, team, alive, false))
        .collect();
    let tox = reg.plan_due_toxin_ticks(field.spawn_frame, &tox_objects);
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
    assert_eq!(
        SpectreGunshipScienceTier::Level1.orbit_duration_frames(),
        300
    );
    assert_eq!(
        SpectreGunshipScienceTier::Level2.orbit_duration_frames(),
        450
    );
    assert_eq!(
        SpectreGunshipScienceTier::Level3.orbit_duration_frames(),
        600
    );
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
    assert_eq!(
        reg.get(id).unwrap().spectre_tier,
        SpectreGunshipScienceTier::Level3
    );
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
        A10StrikeScienceTier::Level1,
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

    // Tick 1: base interval scheduled after (no ROF bonus application).
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.gattling_consecutive, 1);
        assert_eq!(f.howitzer_consecutive, 1);
        assert_eq!(f.gattling_fire_level, 0);
        assert_eq!(f.gattling_rof_mean_applications, 0);
        assert_eq!(f.gattling_rof_fast_applications, 0);
        assert_eq!(f.next_gattling_tick_frame, spawn + 3);
        assert_eq!(f.next_tick_frame, spawn + 9);
    }

    // Tick 2 at spawn+3: consecutive → MEAN for gattling (WeaponBonus 200%).
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 3);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.gattling_consecutive, 2);
        assert_eq!(f.gattling_fire_level, 1);
        assert_eq!(f.gattling_rof_mean_applications, 1);
        assert_eq!(f.gattling_rof_fast_applications, 0);
        assert_eq!(f.next_gattling_tick_frame, spawn + 3 + 1);
        // Howitzer not due at +3 (next is spawn+9).
        assert_eq!(f.howitzer_consecutive, 1);
    }
    assert!(reg.honesty_gattling_continuous_fire_ok());

    // Third gattling tick → FAST + VoiceRapidFire residual cue (WeaponBonus 300%).
    reg.record_orbit_tick_complete(field_id, 90.0, 1, 0, spawn + 4);
    {
        let f = &reg.orbit_fields()[0];
        assert_eq!(f.gattling_consecutive, 3);
        assert_eq!(f.gattling_fire_level, 2);
        assert_eq!(f.gattling_rof_mean_applications, 1);
        assert_eq!(f.gattling_rof_fast_applications, 1);
        assert!(f.rapid_fire_voice_cues >= 1);
    }
    assert!(reg.honesty_voice_rapid_fire_ok());
    assert_eq!(
        SPECTRE_VOICE_RAPID_FIRE_AUDIO,
        "SpectreGunshipVoiceRapidFire"
    );
    assert!(reg.honesty_model_condition_continuous_fire_ok());
    assert!(reg.orbit_fields()[0].model_condition_mean_sets >= 1);
    assert!(reg.orbit_fields()[0].model_condition_fast_sets >= 1);
    assert!(honesty_gattling_weapon_bonus_rof());
    assert!(reg.honesty_gattling_weapon_bonus_rof_ok());

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
