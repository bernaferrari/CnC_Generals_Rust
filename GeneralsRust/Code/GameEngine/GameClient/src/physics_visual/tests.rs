//! Apply-gate + per-appearance calc regressions.

use super::{
    CPP_PI, LocomotorVisualParams, OverlapVisualTarget, PhysicsVisualAppearance, PhysicsVisualBody,
    PhysicsVisualInput, PhysicsVisualLocoState, PhysicsVisualXform, ScriptedClientRng,
    apply_physics_visual_xform, calc_physics_visual_xform, post_multiply_physics_visual_xform,
};
use crate::drawable::{Matrix4, Vector3};

fn enabled_input() -> PhysicsVisualInput {
    PhysicsVisualInput {
        has_object: true,
        show_client_physics: true,
        calculated_xform: Some(PhysicsVisualXform {
            total_pitch: 0.43,
            total_roll: -0.58,
            total_yaw: 0.91,
            total_z: 0.71,
        }),
        ..PhysicsVisualInput::default()
    }
}

fn assert_matrix_close(actual: Matrix4, expected: Matrix4) {
    for row in 0..4 {
        for column in 0..4 {
            let difference = (actual.elements[row][column] - expected.elements[row][column]).abs();
            assert!(
                difference <= 0.000_001,
                "matrix element [{row}][{column}] differs: actual={}, expected={}",
                actual.elements[row][column],
                expected.elements[row][column]
            );
        }
    }
}

fn cpp_translate_z(matrix: &mut Matrix4, z: f32) {
    for row in 0..3 {
        matrix.elements[row][3] += matrix.elements[row][2] * z;
    }
}

fn cpp_rotate_x(matrix: &mut Matrix4, theta: f32) {
    let (sine, cosine) = theta.sin_cos();
    for row in 0..3 {
        let first = matrix.elements[row][1];
        let second = matrix.elements[row][2];
        matrix.elements[row][1] = cosine * first + sine * second;
        matrix.elements[row][2] = -sine * first + cosine * second;
    }
}

fn cpp_rotate_y(matrix: &mut Matrix4, theta: f32) {
    let (sine, cosine) = theta.sin_cos();
    for row in 0..3 {
        let first = matrix.elements[row][0];
        let second = matrix.elements[row][2];
        matrix.elements[row][0] = cosine * first - sine * second;
        matrix.elements[row][2] = sine * first + cosine * second;
    }
}

fn cpp_rotate_z(matrix: &mut Matrix4, theta: f32) {
    let (sine, cosine) = theta.sin_cos();
    for row in 0..3 {
        let first = matrix.elements[row][0];
        let second = matrix.elements[row][1];
        matrix.elements[row][0] = cosine * first + sine * second;
        matrix.elements[row][1] = -sine * first + cosine * second;
    }
}

#[test]
fn physics_visual_gate_matches_drawable_apply_physics_xform() {
    let enabled = enabled_input();
    assert!(enabled.permits_application());

    let cases = [
        (
            PhysicsVisualInput {
                has_object: false,
                ..enabled
            },
            false,
        ),
        (
            PhysicsVisualInput {
                object_disabled_held: true,
                ..enabled
            },
            false,
        ),
        (
            PhysicsVisualInput {
                show_client_physics: false,
                ..enabled
            },
            false,
        ),
        (
            PhysicsVisualInput {
                tactical_view_time_frozen: true,
                camera_movement_finished: false,
                ..enabled
            },
            false,
        ),
        (
            PhysicsVisualInput {
                tactical_view_time_frozen: true,
                camera_movement_finished: true,
                ..enabled
            },
            true,
        ),
        (
            PhysicsVisualInput {
                script_time_frozen_debug: true,
                ..enabled
            },
            false,
        ),
        (
            PhysicsVisualInput {
                script_time_frozen_script: true,
                ..enabled
            },
            false,
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(input.permits_application(), expected, "input={input:?}");
    }
}

#[test]
fn physics_visual_preserves_base_without_calculation_or_when_gated() {
    let base = Matrix4::translation(Vector3::new(3.0, -5.0, 7.0)).mul(&Matrix4::rotation_z(0.37));

    let no_calculation = PhysicsVisualInput {
        calculated_xform: None,
        ..enabled_input()
    };
    assert_eq!(apply_physics_visual_xform(base, no_calculation), base);

    let disabled = PhysicsVisualInput {
        object_disabled_held: true,
        ..enabled_input()
    };
    assert_eq!(apply_physics_visual_xform(base, disabled), base);
}

#[test]
fn physics_visual_uses_cpp_postmultiply_order_and_roll_sign() {
    let base = Matrix4::translation(Vector3::new(3.0, -5.0, 7.0))
        .mul(&Matrix4::rotation_z(0.37))
        .mul(&Matrix4::rotation_x(-0.22));
    let input = enabled_input();
    let xform = input
        .calculated_xform
        .expect("enabled test input has xform");

    let mut expected = base;
    cpp_translate_z(&mut expected, xform.total_z);
    cpp_rotate_y(&mut expected, xform.total_pitch);
    cpp_rotate_x(&mut expected, -xform.total_roll);
    cpp_rotate_z(&mut expected, xform.total_yaw);

    assert_matrix_close(post_multiply_physics_visual_xform(base, xform), expected);
    assert_matrix_close(apply_physics_visual_xform(base, input), expected);
}

#[test]
fn physics_visual_applies_cpp_denormal_hotfix_before_transform() {
    let base = Matrix4::translation(Vector3::new(3.0, -5.0, 7.0));
    let input = PhysicsVisualInput {
        calculated_xform: Some(PhysicsVisualXform {
            total_pitch: 0.5e-20,
            total_roll: -0.5e-20,
            total_yaw: 0.5e-20,
            total_z: -0.5e-20,
        }),
        ..enabled_input()
    };

    assert_eq!(
        input
            .calculated_xform
            .expect("test input has xform")
            .without_denormals(),
        PhysicsVisualXform::default()
    );
    assert_eq!(apply_physics_visual_xform(base, input), base);
}

#[test]
fn pause_does_not_gate_physics_visuals() {
    let enabled = enabled_input();
    assert!(enabled.permits_application());
}

#[test]
fn legs_climber_other_return_none() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![0]);
    for appearance in [
        PhysicsVisualAppearance::LegsTwo,
        PhysicsVisualAppearance::Climber,
        PhysicsVisualAppearance::Other,
    ] {
        assert!(
            calc_physics_visual_xform(appearance, &mut loco, &params, &body, &mut rng).is_none()
        );
    }
}

#[test]
fn thrust_wobble_flips_at_max_and_min() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams {
        wobble_rate: 0.1,
        max_wobble: 0.25,
        min_wobble: -0.25,
        ..LocomotorVisualParams::default()
    };
    let body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![]);

    let first = calc_physics_visual_xform(
        PhysicsVisualAppearance::Thrust,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("thrust has xform");
    assert!((first.total_pitch - 0.1).abs() < 1e-6);
    assert_eq!(loco.wobble, 1.0);

    // Near the max end, rate halves then flips the wobble sign.
    loco.pitch = 0.21;
    let _ = calc_physics_visual_xform(
        PhysicsVisualAppearance::Thrust,
        &mut loco,
        &params,
        &body,
        &mut rng,
    );
    assert_eq!(loco.wobble, -1.0);

    loco.pitch = -0.21;
    let _ = calc_physics_visual_xform(
        PhysicsVisualAppearance::Thrust,
        &mut loco,
        &params,
        &body,
        &mut rng,
    );
    assert_eq!(loco.wobble, 1.0);
}

#[test]
fn hover_spring_converges_and_motive_pitches_from_forward_vel() {
    let mut loco = PhysicsVisualLocoState {
        pitch: 0.4,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams {
        pitch_stiffness: 0.2,
        pitch_damping: 0.5,
        uniform_axial_damping: 1.0,
        forward_vel_coef: 0.1,
        rudder_correction_degree: 0.0,
        elevator_correction_degree: 0.0,
        ..LocomotorVisualParams::default()
    };
    let mut body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![]);

    let before = loco.pitch;
    let _ = calc_physics_visual_xform(
        PhysicsVisualAppearance::Hover,
        &mut loco,
        &params,
        &body,
        &mut rng,
    );
    assert!(
        loco.pitch.abs() < before.abs(),
        "spring should pull pitch toward 0, was {before} now {}",
        loco.pitch
    );

    loco = PhysicsVisualLocoState::default();
    body.is_motive = true;
    body.vel_x = 4.0;
    body.dir_x = 1.0;
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Hover,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("hover");
    // Motive forward vel subtracts FORWARD_VEL_COEFF * forwardVel from pitch
    // after totals are captured, so the stored pitch is negative.
    assert!(loco.pitch < 0.0);
    assert_eq!(info.total_z, 0.0);
}

#[test]
fn hover_rudder_elevator_modulators_advance() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams {
        rudder_correction_degree: 0.5,
        rudder_correction_rate: 0.25,
        elevator_correction_degree: 0.4,
        elevator_correction_rate: 0.1,
        ..LocomotorVisualParams::default()
    };
    let body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Wings,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("wings");
    assert!((loco.yaw_modulator - 0.25).abs() < 1e-6);
    assert!((loco.pitch_modulator - 0.1).abs() < 1e-6);
    assert!((info.total_yaw - 0.5 * 0.25_f32.sin()).abs() < 1e-6);
    assert!((info.total_pitch - 0.4 * 0.1_f32.cos()).abs() < 1e-6);
}

#[test]
fn treads_overlap_mount_and_z_is_half_pre_integrate() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody {
        bounding_circle_radius: 5.0,
        current_overlap: Some(OverlapVisualTarget {
            is_shrubbery: false,
            is_low_overlappable: false,
            is_infantry: false,
            front_crushed: false,
            back_crushed: false,
            pos_x: 1.0,
            pos_y: 0.0,
            bounding_circle_radius: 5.0,
            max_height_above_position: 4.0,
        }),
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::reals(vec![0.0, 0.0, 0.0, 0.0]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Treads,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("treads");
    // amount >= 0.5 → sit on top, overlapZ = height = 4, visual Z = 4/2 = 2
    // before gravity. Stored overlap then gets -0.2 gravity.
    assert!((info.total_z - 2.0).abs() < 1e-5);
    assert!((loco.overlap_z - 3.8).abs() < 1e-5);
    assert!((loco.overlap_z_vel + 0.2).abs() < 1e-5);
}

#[test]
fn treads_leave_overlap_kicks_pitch_rate() {
    let mut loco = PhysicsVisualLocoState {
        overlap_z: 1.0,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody {
        current_overlap: None,
        previous_overlap_valid: true,
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::ints(vec![]);
    let _ = calc_physics_visual_xform(
        PhysicsVisualAppearance::Treads,
        &mut loco,
        &params,
        &body,
        &mut rng,
    );
    assert!(loco.pitch_rate > 0.0);
}

#[test]
fn wheels_airborne_freezes_orientation_and_returns() {
    let mut loco = PhysicsVisualLocoState {
        pitch: 0.3,
        roll: 0.2,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams {
        has_suspension: true,
        max_wheel_extension: -2.0,
        ..LocomotorVisualParams::default()
    };
    let body = PhysicsVisualBody {
        significantly_above_terrain: true,
        pos_z: 20.0,
        terrain_height: 0.0,
        major_radius: 4.0,
        minor_radius: 2.0,
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::ints(vec![0]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::WheelsFour,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("wheels");
    assert_eq!(info.total_pitch, 0.0);
    assert_eq!(info.total_roll, 0.0);
    assert_eq!(info.total_yaw, 0.0);
    assert!(info.total_z > 0.0);
    // Chassis state is not integrated while airborne.
    assert!((loco.pitch - 0.3).abs() < 1e-6);
}

#[test]
fn wheels_suspension_smooths_angle_by_ten() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams {
        has_suspension: true,
        wheel_turn_angle: 0.4,
        max_wheel_extension: -2.0,
        ..LocomotorVisualParams::default()
    };
    let body = PhysicsVisualBody {
        turning: 1,
        forward_speed_2d: 5.0,
        major_radius: 4.0,
        minor_radius: 2.0,
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::ints(vec![0]);
    let _ = calc_physics_visual_xform(
        PhysicsVisualAppearance::WheelsFour,
        &mut loco,
        &params,
        &body,
        &mut rng,
    );
    assert!((loco.wheel_angle - 0.04).abs() < 1e-5);
}

#[test]
fn motorcycle_total_roll_is_always_zero() {
    let mut loco = PhysicsVisualLocoState {
        roll: 0.8,
        acceleration_roll: 0.2,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![0]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Motorcycle,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("motorcycle");
    assert_eq!(info.total_roll, 0.0);
}

#[test]
fn motorcycle_airborne_does_not_return_and_wipes_z() {
    let mut loco = PhysicsVisualLocoState {
        pitch: 0.3,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams {
        has_suspension: true,
        max_wheel_extension: -2.0,
        pitch_stiffness: 0.2,
        pitch_damping: 0.5,
        ..LocomotorVisualParams::default()
    };
    let body = PhysicsVisualBody {
        significantly_above_terrain: true,
        pos_z: 20.0,
        major_radius: 4.0,
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::ints(vec![0]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Motorcycle,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("motorcycle");
    // Airborne Z is computed then wiped; springs still run (autolevel).
    assert_eq!(info.total_z, 0.0);
    assert!(loco.pitch.abs() < 0.3);
}

#[test]
fn calc_denormal_hotfix_clears_tiny_totals() {
    let mut loco = PhysicsVisualLocoState::default();
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody::default();
    let mut rng = ScriptedClientRng::ints(vec![]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::Thrust,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("thrust");
    assert_eq!(info, PhysicsVisualXform::default());
}

#[test]
fn loco_state_default_wobble_is_one() {
    assert_eq!(PhysicsVisualLocoState::default().wobble, 1.0);
}

#[test]
fn show_client_physics_default_matches_cpp_true() {
    let data = game_engine::common::ini::GlobalData::default();
    assert!(data.show_client_physics);
}

#[test]
fn wheels_z_divisor_formula_above_pi_over_eight() {
    let mut loco = PhysicsVisualLocoState {
        pitch: CPP_PI / 4.0,
        ..PhysicsVisualLocoState::default()
    };
    let params = LocomotorVisualParams::default();
    let body = PhysicsVisualBody {
        major_radius: 4.0,
        minor_radius: 2.0,
        ..PhysicsVisualBody::default()
    };
    let mut rng = ScriptedClientRng::ints(vec![0]);
    let info = calc_physics_visual_xform(
        PhysicsVisualAppearance::WheelsFour,
        &mut loco,
        &params,
        &body,
        &mut rng,
    )
    .expect("wheels");
    assert!(info.total_z > 0.0);
}
