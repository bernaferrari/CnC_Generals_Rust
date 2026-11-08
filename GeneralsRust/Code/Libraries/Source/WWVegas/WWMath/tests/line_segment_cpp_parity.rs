use math_utilities::{LineSegment, Matrix3D, Vector3};

fn close(a: Vector3, b: Vector3) {
    assert!((a - b).length() < 1.0e-5, "{a:?} != {b:?}");
}

#[test]
fn transformation_rotates_direction_without_translation() {
    let line = LineSegment::new(Vector3::ZERO, Vector3::new(2.0, 0.0, 0.0));
    let mut transform = Matrix3D::identity();
    transform.set_translation(Vector3::new(5.0, 6.0, 7.0));
    let moved = LineSegment::from_transformed(&line, &transform);
    close(moved.get_p0(), Vector3::new(5.0, 6.0, 7.0));
    close(moved.get_p1(), Vector3::new(7.0, 6.0, 7.0));
    close(moved.get_direction(), Vector3::X);
    close(moved.find_point_closest_to(Vector3::new(6.0, 9.0, 7.0)), Vector3::new(6.0, 6.0, 7.0));
    assert_eq!(moved.get_length(), 2.0);
}

#[test]
fn normalization_preserves_zero_and_normalizes_tiny_nonzero_segments() {
    let zero = LineSegment::new(Vector3::ZERO, Vector3::ZERO);
    assert_eq!(zero.get_direction(), Vector3::ZERO);
    let tiny = LineSegment::new(Vector3::ZERO, Vector3::new(0.0, 0.000001, 0.0));
    close(tiny.get_direction(), Vector3::Y);
    close(tiny.find_point_closest_to(Vector3::new(1.0, 0.0000005, 0.0)), Vector3::new(0.0, 0.0000005, 0.0));
}

#[test]
fn intersection_uses_exact_parallel_check_and_first_length_for_both_fractions() {
    let first = LineSegment::new(Vector3::ZERO, Vector3::new(2.0, 0.0, 0.0));
    let second = LineSegment::new(Vector3::new(1.0, -4.0, 0.0), Vector3::new(1.0, 4.0, 0.0));
    let (ok, p1, t1, p2, t2) = first.find_intersection(&second);
    assert!(ok);
    close(p1, Vector3::X);
    close(p2, Vector3::X);
    assert_eq!(t1, 0.5);
    assert_eq!(t2, 2.0);

    let nearly_parallel = LineSegment::new(Vector3::new(0.0, 1.0, 0.0), Vector3::new(1.0, 1.001, 0.0));
    assert!(first.find_intersection(&nearly_parallel).0);
    let parallel = LineSegment::new(Vector3::Y, Vector3::new(2.0, 1.0, 0.0));
    assert!(!first.find_intersection(&parallel).0);
    assert!(!first.find_intersection(&LineSegment::new(Vector3::ZERO, Vector3::ZERO)).0);
}

#[test]
fn random_segments_consume_shared_rng_state() {
    let mut rng = math_utilities::WwMathRng::new(7);
    let mut expected_rng = rng;
    let mut line = LineSegment::default();
    line.set_random(Vector3::ZERO, Vector3::ONE, &mut rng);
    let expected_p0 = Vector3::new(
        math_utilities::WWMath::random_float(&mut expected_rng),
        math_utilities::WWMath::random_float(&mut expected_rng),
        math_utilities::WWMath::random_float(&mut expected_rng),
    );
    let expected_p1 = Vector3::new(
        math_utilities::WWMath::random_float(&mut expected_rng),
        math_utilities::WWMath::random_float(&mut expected_rng),
        math_utilities::WWMath::random_float(&mut expected_rng),
    );
    assert_eq!(line.get_p0(), expected_p0);
    assert_eq!(line.get_p1(), expected_p1);
    assert_eq!(math_utilities::WWMath::random_float(&mut rng), math_utilities::WWMath::random_float(&mut expected_rng));
}
