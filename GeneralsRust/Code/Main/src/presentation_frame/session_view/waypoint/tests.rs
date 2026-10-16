//! Executed original-C++ corpus under explicit matched unfused arithmetic.
//! Source: W3DView.cpp + ParabolicEase.cpp extracted in packet449 oracle/v2.
//! These are component assertions, not full native-display or retail evidence.
use super::*;
#[path = "corpus.rs"]
mod corpus;

#[test]
fn original_cpp_matched_policy_78_records_have_exact_bits_and_events() {
    let expected: Vec<serde_json::Value> = include_str!("matched-cpp-unfused.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let mut actual = Vec::new();
    corpus::all_cases(&mut actual);
    assert_eq!(actual.len(), 78);
    assert_eq!(expected.len(), actual.len());
    for (expected, actual) in expected.iter().zip(&actual) {
        compare_value(expected, actual, "", None);
    }
}
fn compare_value(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    path: &str,
    key: Option<&str>,
) {
    use serde_json::Value;
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, value) in a {
                compare_value(value, &b[key], &format!("{path}.{key}"), Some(key));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare_value(a, b, &format!("{path}[{i}]"), key);
            }
        }
        (Value::Number(a), Value::Number(b)) if key.is_some_and(is_float_field) => {
            let a = a.as_f64().unwrap() as f32;
            let b = b.as_f64().unwrap() as f32;
            // JSON formatting may change negative-zero spelling; original
            // output and Rust both retain sign through the floating parser.
            assert_eq!(a.to_bits(), b.to_bits(), "{path}: {a:?} versus {b:?}");
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}
fn is_float_field(key: &str) -> bool {
    matches!(
        key,
        "angle"
            | "start_angle"
            | "ground_level"
            | "segment_distance"
            | "total_distance"
            | "position"
            | "points"
            | "angles"
            | "ground"
            | "segment_length"
            | "camera_offset"
            | "previous_position"
    )
}
#[test]
fn full_waypoint_capacity_keeps_safe_padding_and_terminal_phase() {
    let chain: Vec<_> = (1..=25)
        .map(|n| Point {
            x: n as f32 * 100.,
            y: 0.,
            z: 0.,
        })
        .collect();
    let mut state = WaypointMotion::along_chain(
        &chain,
        Point::default(),
        0.,
        10.,
        20.,
        1,
        100,
        0,
        33,
        true,
        0.,
        0.,
        10.,
    );
    assert_eq!(state.num_waypoints, 25);
    assert_eq!(state.points[26].x, 2500.);
    assert_eq!(state.grounds[26], 20.);
    state.validate().unwrap();
    let (mut x, mut y, mut z, mut yaw, mut ground, mut speed) = (0., 0., 0., 0., 10., 1.);
    let mut offset = glam::Vec3::new(0., -100., 100.);
    let (mut frozen, mut arrived, mut constraint) = (true, false, None);
    let mut view = View {
        position_x: &mut x,
        position_y: &mut y,
        position_z: &mut z,
        angle: &mut yaw,
        ground: &mut ground,
        camera_offset: &mut offset,
        basis_pitch_degrees: 37.5,
        basis_yaw_degrees: 0.,
        time_multiplier: &mut speed,
        frozen: &mut frozen,
        arrived: &mut arrived,
        constraint: &mut constraint,
    };
    assert_eq!(state.step(100, &mut view, false), Step::SegmentTerminal);
    state.validate().unwrap();
    assert_eq!(state.step(1, &mut view, false), Step::Finished);
    state.validate().unwrap();
    assert_eq!(*view.position_x, 2400.);
    assert!(!*view.frozen);
    assert!(!*view.arrived);
}
#[test]
fn continuation_validator_rejects_bad_indices_and_accepts_cached_retarget_lengths() {
    WaypointMotion::default().validate().unwrap();
    let mut state = WaypointMotion::move_to(
        Point {
            x: 100.,
            y: 0.,
            z: 0.,
        },
        Point::default(),
        0.,
        10.,
        20.,
        1,
        100,
        true,
        0.,
        0.,
    );
    state.validate().unwrap();
    let lengths = state.lengths;
    state.final_move_to(Point {
        x: 200.,
        y: 90.,
        z: 7.,
    });
    assert_eq!(state.lengths, lengths);
    state.validate().unwrap();
    let mut invalid = state.clone();
    invalid.segment = 27;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.num_waypoints = 26;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.total_time_ms = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.segment = invalid.num_waypoints;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.lengths[invalid.segment] = 0.0;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.elapsed_ms = invalid.total_time_ms + 1;
    assert!(invalid.validate().is_err());
    let mut invalid = state.clone();
    invalid.angles[0] = f32::NAN;
    assert!(invalid.validate().is_err());
    let encoded = bincode_legacy::serialize(&state).unwrap();
    let restored: WaypointMotion = bincode_legacy::deserialize(&encoded).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.lengths, state.lengths);
    assert_eq!(restored.points[2].x.to_bits(), state.points[2].x.to_bits());
}

mod camera_mod_look_tests {
    use super::*;
    fn padded_path() -> WaypointMotion {
        WaypointMotion::along_chain(
            &[
                Point {
                    x: 100.,
                    y: 0.,
                    z: 0.,
                },
                Point {
                    x: 200.,
                    y: 0.,
                    z: 0.,
                },
                Point {
                    x: 300.,
                    y: 0.,
                    z: 0.,
                },
            ],
            Point::default(),
            0.5,
            10.,
            20.,
            1,
            4000,
            0,
            33,
            true,
            0.,
            0.,
            10.,
        )
    }
    #[test]
    fn final_look_toward_skips_early_path_segments() {
        let mut path = padded_path();
        let before = path.angles;
        path.final_look_toward(Point {
            x: 300.,
            y: 200.,
            z: 0.,
        });
        // Source min=max(numWaypoints-1,2). Earlier tables stay intact;
        // penultimate table gets half the wrapped delta, final gets full yaw.
        assert_eq!(&path.angles[..3], &before[..3]);
        assert_ne!(path.angles[3].to_bits(), before[3].to_bits());
        assert_eq!(path.angles[4].to_bits(), 0.0f32.to_bits());
        assert_eq!(path.angles[5].to_bits(), before[5].to_bits());
    }
    #[test]
    fn look_toward_rewrites_every_remaining_segment() {
        let mut path = padded_path();
        path.angles[2..=4].copy_from_slice(&[0.1, 0.2, 0.3]);
        let before = path.angles;
        let (mut x, mut y, mut z, mut angle, mut ground, mut speed) = (0., 0., 0., 2., 10., 1.);
        let mut offset = glam::Vec3::new(0., -100., 100.);
        let (mut frozen, mut arrived, mut constraint) = (false, false, None);
        let mut view = View {
            position_x: &mut x,
            position_y: &mut y,
            position_z: &mut z,
            angle: &mut angle,
            ground: &mut ground,
            camera_offset: &mut offset,
            basis_pitch_degrees: 37.5,
            basis_yaw_degrees: 0.,
            time_multiplier: &mut speed,
            frozen: &mut frozen,
            arrived: &mut arrived,
            constraint: &mut constraint,
        };
        path.look_toward(
            Point {
                x: 400.,
                y: 0.,
                z: 0.,
            },
            &mut view,
            false,
        );
        assert_eq!(&path.angles[..2], &before[..2]);
        for angle in &path.angles[2..=4] {
            assert_eq!(angle.to_bits(), (-std::f32::consts::FRAC_PI_2).to_bits());
        }
        assert_eq!(path.angles[5].to_bits(), before[5].to_bits());
        assert_eq!(
            *view.angle, 2.,
            "modifier must not advance the receiving view before admission"
        );
    }
    #[test]
    fn freeze_angles_to_start_uses_start_yaw() {
        let mut path = padded_path();
        let padded = path.angles[5];
        path.segment = 2;
        path.freeze_angle();
        for angle in &path.angles[1..=4] {
            assert_eq!(angle.to_bits(), 0.5f32.to_bits());
        }
        assert_eq!(path.angles[0].to_bits(), 0.5f32.to_bits());
        assert_eq!(path.angles[5].to_bits(), padded.to_bits());
    }
}

#[test]
fn source_xyz_world_conversion_preserves_raw_waypoint_height() {
    let world = glam::Vec3::new(11., 37.5, 22.);
    let point = Point::from_world(world);
    assert_eq!([point.x, point.y, point.z], [11., 22., 37.5]);
    assert_eq!(point.world(), world);
}
