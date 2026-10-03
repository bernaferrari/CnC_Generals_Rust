use super::*;
use glam::Vec3;

#[test]
fn cpp_particle_enum_ordinals_and_keyframe_limits() {
    assert_eq!(MAX_KEYFRAMES, 8);
    assert_eq!(MAX_VOLUME_PARTICLE_DEPTH, 16);
    assert_eq!(DEFAULT_VOLUME_PARTICLE_DEPTH, 0);
    assert_eq!(OPTIMUM_VOLUME_PARTICLE_DEPTH, 6);
    assert_eq!(ParticleShaderType::Invalid as u32, 0);
    assert_eq!(ParticleShaderType::Multiply as u32, 4);
    assert_eq!(ParticleType::Smudge as u32, 5);
    assert_eq!(EmissionVelocityType::Outward as u32, 5);
    assert_eq!(EmissionVolumeType::Cylinder as u32, 5);
    assert_eq!(WindMotion::NotUsed as u32, 1);
    assert_eq!(WindMotion::Circular as u32, 3);
    for index in 0..=13 {
        assert_eq!(
            ParticlePriorityType::from_index(index).unwrap() as usize,
            index
        );
    }
    assert_eq!(ParticlePriorityType::from_index(14), None);
    assert_eq!(ParticlePriorityType::WeaponTrail as u32, 10);
}

#[test]
fn empty_random_ranges_do_not_consume_the_callers_stream() {
    let mut draws = 0;
    let mut stream = |_, _| {
        draws += 1;
        5.0
    };
    assert_eq!(
        GameClientRandomVariable::new(3.0, 3.0).sample_with(&mut stream),
        3.0
    );
    assert_eq!(
        GameClientRandomVariable::new(8.0, 2.0).sample_with(&mut stream),
        2.0
    );
    assert_eq!(
        GameClientRandomVariable::new(2.0, 8.0).sample_with(&mut stream),
        5.0
    );
    assert_eq!(draws, 1);
    assert_eq!(
        GameClientRandomVariable {
            min: 2.0,
            max: 8.0,
            distribution_type: 5,
        }
        .sample_with(|_, _| panic!("unsupported distributions must not draw")),
        0.0
    );
}

#[test]
fn cpp_keyframe_and_emission_defaults_are_preserved() {
    let key = Keyframe::default();
    assert_eq!((key.value, key.frame), (0.0, 0));
    let rgb = RGBColorKeyframe::default();
    assert_eq!(rgb.color, [0.0; 3]);
    assert_eq!(rgb.frame, 0);
    let random_key = RandomKeyframe::default();
    assert_eq!((random_key.min_value, random_key.max_value), (0.0, 0.0));
    assert_eq!((random_key.distribution_type, random_key.frame), (0, 0));
    let EmissionVelocity::Ortho { x, y, z } = EmissionVelocity::default() else {
        panic!("default velocity variant changed");
    };
    for component in [x, y, z] {
        assert_eq!(
            (component.min, component.max, component.distribution_type),
            (0.0, 0.0, 0)
        );
    }
    assert!(matches!(EmissionVolume::default(), EmissionVolume::Point));
}

#[test]
fn sphere_rejects_only_zero_and_draws_xyz_in_original_order() {
    let mut samples = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0].into_iter();
    let mut bounds = Vec::new();
    let point = point_on_unit_sphere(|low, high| {
        bounds.push((low, high));
        samples.next().unwrap()
    });
    assert_eq!(bounds, vec![(-1.0, 1.0); 6]);
    // (1,1,1) lies outside the unit ball and is nevertheless accepted in C++.
    assert_eq!(point, Vec3::ONE.normalize());
    assert_eq!(samples.next(), None);
}

#[test]
fn explicit_random_sources_are_isolated_when_interleaved() {
    let mut first = [1.0, 0.0, 0.0].into_iter();
    let mut second = [0.0, 0.0, 1.0].into_iter();
    let mut second_bounds = Vec::new();
    assert_eq!(point_on_unit_sphere(|_, _| first.next().unwrap()), Vec3::X);
    assert_eq!(
        point_on_unit_hemisphere(|low, high| {
            second_bounds.push((low, high));
            second.next().unwrap()
        }),
        Vec3::Z
    );
    assert_eq!(second_bounds, [(-1.0, 1.0), (-1.0, 1.0), (0.0, 1.0)]);
    assert_eq!(first.next(), None);
    assert_eq!(second.next(), None);
}

#[test]
fn translation_preserves_acceleration_damping_drift_order() {
    let mut position = Vec3::new(10.0, 20.0, 30.0);
    let mut velocity = Vec3::new(2.0, 4.0, 6.0);
    integrate_translation(
        &mut position,
        &mut velocity,
        Vec3::new(2.0, -2.0, 4.0),
        0.5,
        Vec3::new(1.0, 2.0, 3.0),
    );
    assert_eq!(velocity, Vec3::new(2.0, 1.0, 5.0));
    assert_eq!(position, Vec3::new(13.0, 23.0, 38.0));
    let mut independent_position = Vec3::ZERO;
    let mut independent_velocity = Vec3::ZERO;
    integrate_translation(
        &mut independent_position,
        &mut independent_velocity,
        Vec3::ZERO,
        1.0,
        Vec3::Y,
    );
    assert_eq!(independent_position, Vec3::Y);
    assert_eq!(position, Vec3::new(13.0, 23.0, 38.0));
}

#[test]
fn wind_uses_emitter_distance_and_position_not_acceleration() {
    let mut near = Vec3::new(75.0, 0.0, 0.0);
    apply_wind_motion(&mut near, 0.0, Vec3::ZERO, 0.5);
    assert_eq!(near, Vec3::new(76.0, 0.0, 0.0));
    let mut halfway = Vec3::new(137.5, 0.0, 0.0);
    apply_wind_motion(&mut halfway, 0.0, Vec3::ZERO, 0.5);
    assert_eq!(halfway, Vec3::new(138.0, 0.0, 0.0));
    let mut edge = Vec3::new(200.0, 0.0, 0.0);
    apply_wind_motion(&mut edge, 0.0, Vec3::ZERO, 0.5);
    assert_eq!(edge, Vec3::new(200.0, 0.0, 0.0));
    apply_wind_motion(&mut edge, 0.0, Vec3::new(200.0, 0.0, 0.0), 0.5);
    assert_eq!(edge, Vec3::new(201.0, 0.0, 0.0));
}
