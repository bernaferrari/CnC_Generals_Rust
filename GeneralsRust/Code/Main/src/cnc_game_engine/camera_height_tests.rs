//! Actual source-basis/HAG operations, not a second camera implementation.
use super::*;

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.000_02,
        "{actual} != {expected}"
    );
}
fn vec_near(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 0.002,
        "{actual:?} != {expected:?}"
    );
}
fn rules() -> HeightRules {
    HeightRules {
        min: 200.,
        max: 900.,
        enforce_max: true,
        adjust_speed: 0.3,
        scroll_cutoff: 10.,
    }
}
fn step() -> HeightStep {
    HeightStep {
        terrain: 100.,
        paused: false,
        scrolling: false,
        scroll_amount: 0.,
        scripted: false,
        replay_camera: false,
        rules: rules(),
    }
}
fn owner() -> CameraHeight {
    let mut owner = CameraHeight::new(1000., 45., 0.);
    owner.initialize_map(0., 1000., 45., 0.);
    owner.set_adjustment_enabled(true);
    owner.set_height(400., false, 0., 0.);
    owner
}

#[test]
fn idle_height_adjusts_zoom_only_and_keeps_the_old_goal() {
    let mut height = owner();
    let mut zoom = 0.8;
    assert!(height.advance(&mut zoom, step()));
    near(zoom, 0.71);
    assert_eq!(height.desired(), 400.);
    assert_eq!(height.basis().offset.z, 1000.);
}

#[test]
fn scripted_and_replay_capture_current_height_after_old_goal_calculation() {
    let mut height = owner();
    let mut zoom = 0.8;
    assert!(!height.advance(
        &mut zoom,
        HeightStep {
            scripted: true,
            ..step()
        }
    ));
    assert_eq!((zoom, height.desired()), (0.8, 700.));
    let mut scrolling = owner();
    let mut zoom = 0.8;
    assert!(scrolling.advance(
        &mut zoom,
        HeightStep {
            scripted: true,
            scrolling: true,
            ..step()
        }
    ));
    near(zoom, 0.71);
    assert_eq!(
        scrolling.desired(),
        700.,
        "scrolling integrates toward the OLD goal, even during scripted motion"
    );
    let mut replay = owner();
    let mut zoom = 0.8;
    assert!(replay.advance(
        &mut zoom,
        HeightStep {
            replay_camera: true,
            ..step()
        }
    ));
    near(zoom, 0.71);
    assert_eq!(replay.desired(), 700.);
}

#[test]
fn adjustment_enabled_and_pause_guard_goal_as_well_as_zoom() {
    for paused in [false, true] {
        let mut height = owner();
        height.set_adjustment_enabled(true);
        let mut zoom = 0.8;
        let blocked = if paused {
            HeightStep {
                paused: true,
                scripted: true,
                ..step()
            }
        } else {
            height.set_adjustment_enabled(false);
            HeightStep {
                scripted: true,
                ..step()
            }
        };
        assert!(!height.advance(&mut zoom, blocked));
        assert_eq!((zoom, height.desired()), (0.8, 400.));
    }
}

#[test]
fn scrolling_boolean_magnitude_and_height_limits_are_independent_inputs() {
    let mut height = owner();
    let mut zoom = 0.8;
    assert!(!height.advance(
        &mut zoom,
        HeightStep {
            scrolling: true,
            scroll_amount: 10.,
            ..step()
        }
    ));
    assert_eq!(zoom, 0.8);
    assert!(height.advance(
        &mut zoom,
        HeightStep {
            scrolling: false,
            scroll_amount: 100.,
            ..step()
        }
    ));
    near(zoom, 0.71);
    for (current_zoom, enforce, expected_change) in
        [(0.2, false, true), (1.2, false, false), (1.2, true, true)]
    {
        let mut height = owner();
        let mut zoom = current_zoom;
        let mut config = rules();
        config.enforce_max = enforce;
        assert_eq!(
            height.advance(
                &mut zoom,
                HeightStep {
                    scrolling: true,
                    scroll_amount: 100.,
                    rules: config,
                    ..step()
                }
            ),
            expected_change
        );
    }
}

#[test]
fn integration_does_not_invent_positive_speed_or_zoom_clamps() {
    let mut height = owner();
    let mut zoom = 0.8;
    let mut config = rules();
    config.adjust_speed = -1.;
    assert!(height.advance(
        &mut zoom,
        HeightStep {
            rules: config,
            ..step()
        }
    ));
    near(zoom, 1.1);
    let mut height = owner();
    let mut zoom = 0.8;
    config.adjust_speed = 4.;
    assert!(height.advance(
        &mut zoom,
        HeightStep {
            rules: config,
            ..step()
        }
    ));
    near(zoom, -0.4);
    let mut height = owner();
    let mut zoom = 0.8;
    config.adjust_speed = 0.;
    assert!(!height.advance(
        &mut zoom,
        HeightStep {
            rules: config,
            ..step()
        }
    ));
    assert_eq!(zoom, 0.8);
}

#[test]
fn source_adjustment_threshold_is_inclusive() {
    let mut height = owner();
    height.set_height(0., false, 0., 0.);
    let mut zoom = 0.0001;
    let mut config = rules();
    config.adjust_speed = 1.;
    assert!(height.advance(
        &mut zoom,
        HeightStep {
            terrain: 0.,
            rules: config,
            ..step()
        }
    ));
    assert_eq!(zoom, 0.);
    let mut zoom = f32::from_bits(0.0001f32.to_bits() - 1);
    assert!(!height.advance(
        &mut zoom,
        HeightStep {
            terrain: 0.,
            rules: config,
            ..step()
        }
    ));
}

#[test]
fn around_height_uses_five_raw_ordered_samples_and_propagates_absence() {
    let mut visits = Vec::new();
    let values = [10., 20., 80., 30., 40.];
    let sampled = height_around(Vec3::new(100., 999., 200.), |x, z| {
        visits.push((x, z));
        Some(values[visits.len() - 1])
    });
    assert_eq!(sampled, Some(80.));
    assert_eq!(
        visits,
        [
            (100., 200.),
            (140., 160.),
            (60., 160.),
            (140., 240.),
            (60., 240.)
        ]
    );
    let mut count = 0;
    assert_eq!(
        height_around(Vec3::ZERO, |_, _| {
            count += 1;
            (count < 3).then_some(-20.)
        }),
        None
    );
    assert_eq!(
        count, 3,
        "missing sample cannot be replaced by eye or target height"
    );
    assert_eq!(
        height_around(Vec3::ZERO, |x, z| Some(if x < 0. && z > 0. {
            -2.
        } else {
            -8.
        })),
        Some(-2.)
    );
}

#[test]
fn map_basis_caps_only_positive_ground_and_is_independent_of_view_pitch() {
    let high = CameraBasis::for_map(200., 400., 45., 0.);
    assert_eq!((high.ground_level, high.offset.z), (120., 520.));
    let negative = CameraBasis::for_map(-20., 400., 45., 0.);
    assert_eq!((negative.ground_level, negative.offset.z), (-20., 380.));
    let before = negative.offset.z;
    let first = negative.pose(Vec3::new(10., 999., 20.), 0.8, -0.2, 0.);
    let second = negative.pose(Vec3::new(10., -999., 20.), 0.8, 0.2, 1.);
    assert_ne!(first.0, second.0);
    assert_eq!(negative.offset.z, before);
    assert_eq!(first.1.y, -20.);
    assert_eq!(second.1.y, -20.);
}

#[test]
fn source_pose_uses_ground_factor_and_independent_pitch_heading_rotations() {
    let basis = CameraBasis::for_map(100., 400., 45., 0.);
    let (eye, look) = basis.pose(Vec3::new(10., 999., 20.), 1., 0., 0.);
    vec_near(eye, Vec3::new(10., 500., -380.));
    vec_near(look, Vec3::new(10., 100., 20.));
    let (eye, _) = basis.pose(Vec3::new(10., 0., 20.), 1., 0., std::f32::consts::FRAC_PI_2);
    vec_near(eye, Vec3::new(410., 500., 20.));
    let yawed = CameraBasis::for_map(100., 400., 45., 45.);
    let (eye, look) = yawed.pose(Vec3::new(10., 0., 20.), 1., 0., 0.);
    vec_near(eye, Vec3::new(410., 500., -380.));
    vec_near(look, Vec3::new(10., 100., 20.));
}

#[test]
fn native_pose_inverse_recovers_angles_without_recalibrating_map_basis() {
    for ini_yaw in [0., 25.] {
        let basis = CameraBasis::for_map(80., 400., 45., ini_yaw);
        let original = basis.offset;
        for pitch in [-0.3, 0., 0.3] {
            for heading in [-2., 0., 2.] {
                let position = Vec3::new(100., 800., 200.);
                let (eye, look) = basis.pose(position, 0.9, pitch, heading);
                let (restored_pitch, restored_heading) =
                    basis.angles_from_pose(position, eye, 0.9).unwrap();
                assert!((restored_pitch - pitch).abs() < 0.00001);
                assert!((restored_heading - heading).abs() < 0.00001);
                let (restored_eye, restored_look) =
                    basis.pose(position, 0.9, restored_pitch, restored_heading);
                vec_near(restored_eye, eye);
                vec_near(restored_look, look);
                assert_eq!(basis.offset, original);
            }
        }
    }
}

#[test]
fn map_default_set_and_capture_are_owned_and_construction_is_inert() {
    let mut first = owner();
    first.set_height(275., false, 0., 0.);
    let first_pose = first.basis().pose(Vec3::new(10., 0., 20.), 0.8, 0., 0.);
    let mut peer = CameraHeight::new(600., 30., 15.);
    assert_eq!(peer.desired(), 0.);
    peer.initialize_map(-20., 600., 30., 15.);
    peer.set_height(20., true, 50., 500.);
    assert_eq!(peer.desired(), 50.);
    assert_eq!(first.desired(), 275.);
    assert_eq!(
        first.basis().pose(Vec3::new(10., 0., 20.), 0.8, 0., 0.),
        first_pose
    );
    let reset_zoom = peer.set_zoom_to_default(80., 500.);
    near(reset_zoom, 1.);
    assert_eq!(peer.desired(), 500.);
    let unchanged = first.desired();
    let reset_target = first.default_zoom(100., 900.);
    assert_eq!(reset_target, 1.);
    assert_eq!(
        first.desired(),
        unchanged,
        "animated reset target calculation does not write desired HAG"
    );
    let mut restored = CameraHeight::new(1000., 45., 0.);
    restored.initialize_map(0., 1000., 45., 0.);
    restored.set_height(first.desired(), false, 0., 0.);
    assert_eq!(restored.desired(), first.desired());
    restored.set_height(2000., true, 50., 500.);
    assert_eq!(restored.desired(), 500.);
    assert_eq!(first.desired(), 275.);
}
