//! Defined retail quirks plus the explicit memory-safety exception for exhausted keys.
use super::*;

fn visible_particle() -> Particle {
    let mut info = ParticleInfo::default();
    info.lifetime = 0;
    info.color_scale = 0.0;
    info.alpha_keys[0].value = 1.0;
    Particle::new(&info, 17, 0)
}

#[test]
fn particle_retail_green_lower_clamp_reads_already_clamped_red() {
    // ParticleSys.cpp:430-444 clamps RED first; GREEN's lower test then
    // reads that clamped RED, while GREEN's upper test reads GREEN.
    for (before, expected) in [
        ([0.25, -0.5, 1.5], [0.25, -0.5, 1.0]),
        ([-0.5, -0.25, -1.0], [0.0, -0.25, 0.0]),
        ([1.5, 1.25, 0.5], [1.0, 1.0, 0.5]),
    ] {
        let mut particle = visible_particle();
        particle.color = before;
        assert!(particle.update(Vec3::ZERO, 1, ParticleShaderType::Alpha));
        assert_eq!(particle.color, expected);
    }
}

#[test]
fn particle_retail_emitter_angle_preserves_axis_and_zero_vector_branches() {
    use std::f32::consts::{PI, TAU};
    // ParticleSys.cpp:3390-3420 returns PI/0, rather than +/-PI/2, at
    // exact orthogonality to (0,1), and returns zero for a zero vector.
    for (direction, expected) in [
        (Vec3::X, TAU),
        (-Vec3::X, PI),
        (Vec3::Y, PI),
        (-Vec3::Y, 0.0),
        (Vec3::ZERO, PI),
    ] {
        let mut particle = visible_particle();
        particle.particle_up_towards_emitter = true;
        particle.position = direction;
        particle.angle_z = 0.37;
        particle.angular_rate_z = 0.2;
        assert!(particle.update(Vec3::ZERO, 1, ParticleShaderType::Alpha));
        if direction == -Vec3::Y {
            // C++ DEFAULT_TRIG calls acosf(-1): the host math library can
            // leave a one-ulp remainder after -theta + PI. This is not
            // angleBetween's exact orthogonality/zero-vector branch.
            assert!((particle.angle_z - expected).abs() <= 4.0 * f32::EPSILON);
        } else {
            assert_eq!(
                particle.angle_z.to_bits(),
                expected.to_bits(),
                "{direction:?}"
            );
        }
    }
}

#[test]
fn particle_retail_emitter_angle_uses_signed_acos_away_from_axes() {
    use std::f32::consts::{FRAC_PI_4, PI};
    for (direction, expected) in [
        (Vec3::new(1.0, 1.0, 0.0), PI + FRAC_PI_4),
        (Vec3::new(-1.0, 1.0, 0.0), PI - FRAC_PI_4),
        (Vec3::new(1.0, -1.0, 0.0), PI + 3.0 * FRAC_PI_4),
        (Vec3::new(-1.0, -1.0, 0.0), PI - 3.0 * FRAC_PI_4),
    ] {
        let mut particle = visible_particle();
        particle.particle_up_towards_emitter = true;
        particle.position = direction;
        assert!(particle.update(Vec3::ZERO, 1, ParticleShaderType::Alpha));
        assert!((particle.angle_z - expected).abs() <= 4.0 * f32::EPSILON);
    }
}

#[test]
fn particle_exhausted_color_keys_are_safe_without_invented_terminal_color() {
    for target in [MAX_KEYFRAMES, MAX_KEYFRAMES + 1, usize::MAX] {
        let mut particle = visible_particle();
        particle.color_target_key = target;
        particle.color = [0.0; 3];
        assert!(!particle.is_invisible(ParticleShaderType::Additive));
        particle.color = [1.0; 3];
        assert!(!particle.is_invisible(ParticleShaderType::Multiply));
    }
    // Defined in-bounds thresholds retain original behavior.
    let mut particle = visible_particle();
    particle.color_target_key = 1;
    particle.color_keys[1].frame = 0;
    particle.color = [0.02; 3];
    assert!(particle.is_invisible(ParticleShaderType::Additive));
    particle.color = [1.0; 3];
    assert!(particle.is_invisible(ParticleShaderType::Multiply));
    particle.color_keys[1].frame = 1;
    assert!(!particle.is_invisible(ParticleShaderType::Multiply));
}
