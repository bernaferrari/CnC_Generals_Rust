//! FX/particle randomness must come from the seeded GameClient stream.
//!
//! C++ draws every client FX random value through `GameClientRandomValue` /
//! `GameClientRandomValueReal` (RandomValue.cpp:353-371), so identical seeds
//! reproduce identical FX. These regression tests fail when any FX draw falls
//! back to a non-deterministic source (previously `rand::thread_rng` /
//! `rand::random` at the tracer probability, particle spawn angle, random
//! scorch selection, and every emission volume/velocity/burst/wind sample).
//!
//! Both checks live in ONE test on purpose: they reseed the process-global
//! GameClient stream, so parallel test threads would race the stream state.

use std::sync::Arc;

use game_client_rust::effects::particle_manager::{
    EmissionVelocity, EmissionVolume, GameClientRandomVariable, ParticleSystemTemplate,
};
use game_client_rust::effects::particle_system::ParticleSystem;
use game_client_rust::terrain::scorch_mesh::resolve_scorch_type;

#[test]
fn fx_randomness_is_reproducible_from_same_client_seed() {
    use game_engine::common::random_value::init_random_with_seed;

    // --- Random scorch selection (C++ FXList.cpp:433) -----------------------
    // Explicit scorch types pass through untouched.
    assert_eq!(resolve_scorch_type(2), 2);

    init_random_with_seed(20260829);
    let scorch_first = resolve_scorch_type(-1);
    init_random_with_seed(20260829);
    let scorch_second = resolve_scorch_type(-1);

    assert!(
        (0..=3).contains(&scorch_first),
        "scorch {scorch_first} outside SCORCH_1..SCORCH_4"
    );
    assert_eq!(
        scorch_first, scorch_second,
        "same seed must reproduce the same scorch"
    );

    // --- Particle wind + emission (C++ ParticleSys.cpp:1123-1125, 1531-1648)
    let build_system = || {
        let mut template = ParticleSystemTemplate::new("SeededFx".to_string());
        {
            let info = template.info_mut();
            info.emission_volume = EmissionVolume::Sphere { radius: 5.0 };
            info.emission_velocity = EmissionVelocity::Spherical {
                speed: GameClientRandomVariable::new(1.0, 2.0),
            };
            // Fixed burst count and lifetime keep the draw pattern stable; the
            // wind-angle ranges make the constructor draws genuinely
            // stream-dependent (C++ ParticleSys.cpp:1123-1125).
            info.burst_count = GameClientRandomVariable::new(3.0, 3.0);
            info.system_lifetime = 60;
            info.wind_motion_start_angle_min = 10.0;
            info.wind_motion_start_angle_max = 20.0;
            info.wind_motion_end_angle_min = 30.0;
            info.wind_motion_end_angle_max = 40.0;
        }
        ParticleSystem::new(Arc::new(template), 7, false)
    };

    init_random_with_seed(424242);
    let mut system_a = build_system();
    let wind_a = system_a.wind_angle();
    let _ = system_a.begin_frame_emit(0, 10);
    let emissions_a = system_a.take_pending_emissions();

    init_random_with_seed(424242);
    let mut system_b = build_system();
    let wind_b = system_b.wind_angle();
    let _ = system_b.begin_frame_emit(0, 10);
    let emissions_b = system_b.take_pending_emissions();

    assert_eq!(
        emissions_a.len(),
        3,
        "fixed burst count must emit exactly 3 particle infos"
    );
    assert_eq!(
        wind_a, wind_b,
        "same seed must reproduce the constructor wind draw"
    );
    for (a, b) in emissions_a.iter().zip(&emissions_b) {
        assert_eq!(a.position, b.position, "same seed must reproduce position");
        assert_eq!(
            a.velocity, b.velocity,
            "same seed must reproduce velocity"
        );
        assert!(
            a.position.coords.norm().is_finite() && a.velocity.norm().is_finite(),
            "emission samples must be finite"
        );
        assert!(
            a.velocity.norm() > 0.0,
            "spherical speed in [1, 2] must produce non-zero velocity"
        );
    }
}
