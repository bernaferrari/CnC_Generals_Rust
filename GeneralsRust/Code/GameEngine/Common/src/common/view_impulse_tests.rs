//! Existing scalar owners borrowed directly; no singleton fixture or test lock.
use super::*;

fn rules() -> GlobalData {
    let mut rules = GlobalData::default();
    rules.shake_normal_intensity = 2.0;
    rules.max_shake_range = 100.0;
    rules.max_shake_intensity = 10.0;
    rules
}

#[test]
fn enqueue_attenuates_xy_then_update_commits_and_reverses_direction() {
    let (mut intensity, mut cosine, mut sine) = (0.0, 0.0, 0.0);
    let mut draws = Vec::new();
    let mut random = |low, high| {
        draws.push((low, high));
        0.0
    };
    let mut impulse = ViewImpulse {
        intensity: &mut intensity,
        cosine: &mut cosine,
        sine: &mut sine,
    };
    impulse.enqueue(
        [30., 40.],
        [0., 0.],
        CameraShakeType::Normal,
        &rules(),
        &mut random,
    );
    assert_eq!(*impulse.intensity, 1.0);
    assert_eq!((*impulse.cosine, *impulse.sine), (1.0, 0.0));
    assert_eq!(impulse.step(), [1.0, 0.0]);
    assert_eq!(*impulse.intensity, 0.75);
    assert_eq!(impulse.step(), [-0.75, -0.0]);
    assert_eq!(*impulse.intensity, 0.5625);
    assert_eq!(draws, vec![(0.0, 2.0 * std::f32::consts::PI)]);
}

#[test]
fn out_of_range_still_draws_and_replaces_active_direction_without_new_intensity() {
    let (mut intensity, mut cosine, mut sine) = (2.0, 1.0, 0.0);
    let mut count = 0;
    let angle = std::f32::consts::FRAC_PI_2;
    let mut impulse = ViewImpulse {
        intensity: &mut intensity,
        cosine: &mut cosine,
        sine: &mut sine,
    };
    impulse.enqueue(
        [1000., 0.],
        [0., 0.],
        CameraShakeType::Normal,
        &rules(),
        &mut |_, _| {
            count += 1;
            angle
        },
    );
    assert_eq!(count, 1);
    assert_eq!(*impulse.intensity, 2.0);
    // Source promotes Real into double trig, then stores Real.
    assert_eq!(
        impulse.cosine.to_bits(),
        (f64::from(angle).cos() as f32).to_bits()
    );
    assert_eq!(
        impulse.sine.to_bits(),
        (f64::from(angle).sin() as f32).to_bits()
    );
    let expected = [2.0 * *impulse.cosine, 2.0 * *impulse.sine];
    assert_eq!(impulse.step(), expected);
}

#[test]
fn overflow_is_fixed_three_and_quiet_step_preserves_direction() {
    let mut rules = rules();
    rules.shake_normal_intensity = 5.0;
    rules.max_shake_intensity = 4.0;
    let (mut intensity, mut cosine, mut sine) = (0.0, 0.25, -0.5);
    let mut impulse = ViewImpulse {
        intensity: &mut intensity,
        cosine: &mut cosine,
        sine: &mut sine,
    };
    impulse.enqueue(
        [0., 0.],
        [0., 0.],
        CameraShakeType::Normal,
        &rules,
        &mut |_, _| 0.0,
    );
    assert_eq!(*impulse.intensity, 3.0);
    assert_eq!(impulse.step(), [3.0, 0.0]);
    *impulse.intensity = 0.01;
    let direction = (*impulse.cosine, *impulse.sine);
    assert_eq!(impulse.step(), [0.0, 0.0]);
    assert_eq!(*impulse.intensity, 0.0);
    assert_eq!((*impulse.cosine, *impulse.sine), direction);
}

#[test]
fn borrowed_owners_are_independent_across_enqueue_step_and_reconstruction() {
    let (mut first_i, mut first_c, mut first_s) = (0.0, 0.0, 0.0);
    let (mut peer_i, mut peer_c, mut peer_s) = (0.0, 0.0, 0.0);
    ViewImpulse {
        intensity: &mut first_i,
        cosine: &mut first_c,
        sine: &mut first_s,
    }
    .enqueue(
        [0., 0.],
        [0., 0.],
        CameraShakeType::Normal,
        &rules(),
        &mut |_, _| 0.0,
    );
    let saved = (first_i, first_c, first_s);
    ViewImpulse {
        intensity: &mut peer_i,
        cosine: &mut peer_c,
        sine: &mut peer_s,
    }
    .enqueue(
        [0., 0.],
        [0., 0.],
        CameraShakeType::Normal,
        &rules(),
        &mut |_, _| 1.0,
    );
    let peer = ViewImpulse {
        intensity: &mut peer_i,
        cosine: &mut peer_c,
        sine: &mut peer_s,
    }
    .step();
    assert_ne!(peer, [2.0, 0.0]);
    assert_eq!((first_i, first_c, first_s), saved);
    assert_eq!(
        ViewImpulse {
            intensity: &mut first_i,
            cosine: &mut first_c,
            sine: &mut first_s
        }
        .step(),
        [2.0, 0.0]
    );
    assert_eq!(first_i, 1.5);
}
