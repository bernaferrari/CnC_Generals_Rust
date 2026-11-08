use math_utilities::{AABox, Matrix3D, MinMaxAABox, Vector3, WwMathRng, WWMath};

#[test]
fn containment_uses_center_relative_coordinates_at_large_positions() {
    let center = Vector3::new(100_000_000.0, 0.0, 0.0);
    let outer = AABox::new(center, Vector3::new(1.0, 1.0, 1.0));
    let larger = AABox::new(center, Vector3::new(4.0, 1.0, 1.0));
    assert!(!outer.contains_box(&larger));
    assert!(outer.intersects(&larger));
    assert!(outer.contains_box(&outer));
    assert!(outer.contains_point(center + Vector3::Y));
    assert!(!outer.contains_point(center + Vector3::Y * 2.0));
}

#[test]
fn touching_and_degenerate_box_additions_follow_cpp_overloads() {
    let mut center_box = AABox::new(Vector3::ZERO, Vector3::ONE);
    let point_box = AABox::new(Vector3::new(3.0, 0.0, 0.0), Vector3::ZERO);
    assert!(!center_box.intersects(&point_box));
    center_box.add_box(&point_box);
    assert!(center_box.contains_point(point_box.center));
    assert!(center_box.intersects(&point_box));

    let mut minmax = MinMaxAABox::new(-Vector3::ONE, Vector3::ONE);
    let original = minmax;
    minmax.add_aabox(&point_box);
    minmax.add_min_max_box(&MinMaxAABox::new(point_box.center, point_box.center));
    minmax.add_box(point_box.center, point_box.center);
    assert_eq!(minmax, original);
    minmax.add_point(point_box.center);
    assert_eq!(minmax.max_corner.x, 3.0);
}

#[test]
fn transforms_accumulate_translation_before_axis_contributions() {
    let matrix = Matrix3D::from_values(
        1.0, 1.0, 0.0, 1.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
    );
    let point = Vector3::new(100_000_000.0, -100_000_000.0, 0.0);
    let mut center_box = AABox::new(point, Vector3::ZERO);
    center_box.transform(&matrix);
    assert_eq!(center_box.center.x, 0.0);
    assert_eq!(center_box.extent, Vector3::ZERO);

    let mut minmax = MinMaxAABox::new(point, point);
    minmax.transform(&matrix);
    assert_eq!(minmax.min_corner.x, 0.0);
    assert_eq!(minmax.max_corner.x, 0.0);
}

#[test]
fn random_boxes_advance_the_caller_owned_stream() {
    let mut rng = WwMathRng::new(1);
    let mut expected_rng = rng;
    let mut first = AABox::ZERO;
    let mut second = AABox::ZERO;
    first.init_random(0.0, 1.0, 0.0, 1.0, &mut rng);
    second.init_random(0.0, 1.0, 0.0, 1.0, &mut rng);
    let expected = Vector3::new(
        WWMath::random_float(&mut expected_rng),
        WWMath::random_float(&mut expected_rng),
        WWMath::random_float(&mut expected_rng),
    );
    assert_eq!(first.center, expected);
    for _ in 0..3 {
        WWMath::random_float(&mut expected_rng);
    }
    assert_eq!(second.center.x, WWMath::random_float(&mut expected_rng));
    assert_ne!(first, second);
}
