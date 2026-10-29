use math_utilities::{NormalCone, Vector3};

const CPP_EPSILON: f32 = 0.0001;

#[test]
fn complete_sphere_boundary_uses_cpp_subtractive_epsilon() {
    for angle in [-1.0, -1.0 + CPP_EPSILON, -0.99995] {
        assert!(
            NormalCone::new(Vector3::Z, angle).is_complete_sphere(),
            "C++ Complete_Sphere accepts angle {angle}"
        );
    }
    assert!(!NormalCone::new(Vector3::Z, -0.99989).is_complete_sphere());
}

#[test]
fn coplanar_normals_use_the_original_angle_to_radians_mapping() {
    let cone = NormalCone::new(Vector3::Z, 0.5);
    let (length, n1, n2) = cone
        .get_coplanar_normals(&Vector3::X)
        .expect("perpendicular input has coplanar edge normals");

    assert_eq!(length, 1.0);
    let radians = (1.0 - cone.angle) * std::f32::consts::PI * 0.5;
    let center_dot = radians.cos();
    let input_dot = radians.sin();
    assert!((n1.dot(Vector3::Z) - center_dot).abs() <= CPP_EPSILON);
    assert!((n2.dot(Vector3::Z) - center_dot).abs() <= CPP_EPSILON);
    assert!((n1.dot(Vector3::X).abs() - input_dot).abs() <= CPP_EPSILON);
    assert!((n2.dot(Vector3::X).abs() - input_dot).abs() <= CPP_EPSILON);
    assert!((center_dot - 0.70710677).abs() <= CPP_EPSILON);
}

#[test]
fn opposite_normal_merge_keeps_cone_when_cpp_cross_length_short_circuits() {
    let mut cone = NormalCone::from_direction(Vector3::Z);
    cone.merge_normal(&-Vector3::Z);
    assert_eq!(cone.center, Vector3::Z);
    assert_eq!(cone.angle, 1.0);
}

#[test]
fn nearly_opposite_merge_keeps_cone_below_cpp_cross_length_threshold() {
    let mut cone = NormalCone::from_direction(Vector3::Z);
    let input = Vector3::new(0.005, 0.0, -0.9999875);
    cone.merge_normal(&input);
    assert_eq!(cone.center, Vector3::Z);
    assert_eq!(cone.angle, 1.0);
}
