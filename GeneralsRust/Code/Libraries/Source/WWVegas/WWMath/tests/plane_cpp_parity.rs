use math_utilities::{Plane, Vector3, EPSILON};

#[test]
fn three_points_only_fall_back_for_an_exactly_zero_cross_product() {
    let p1 = Vector3::new(0.0, 2.0, 0.0);
    let p2 = Vector3::new(0.001, 2.0, 0.0);
    let p3 = Vector3::new(0.0, 2.0, 0.001);
    let plane = Plane::from_three_points(p1, p2, p3);
    assert!((plane.normal - Vector3::new(0.0, -1.0, 0.0)).length() < EPSILON);
    assert!((plane.distance + 2.0).abs() < EPSILON);
    let (direction, point) = plane.intersect_with_plane(&plane);
    assert_eq!(direction, Vector3::ZERO);
    assert!(point.x.is_nan() && point.y.is_nan());
    assert_eq!(point.z, 0.0);
    let mut updated = Plane::default();
    updated.set_three_points(p1, p2, p3);
    assert_eq!(updated, plane);
    updated.set_three_points(p1, p1, p1);
    assert_eq!(updated, Plane::default());
}

#[test]
fn segment_intersection_uses_exact_parallel_and_inclusive_bounds() {
    let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);
    let p0 = Vector3::new(0.0, -0.000001, 0.0);
    let p1 = Vector3::new(0.0, 0.000001, 0.0);
    assert_eq!(plane.compute_intersection(p0, p1), (true, 0.5));
    assert_eq!(plane.compute_intersection(Vector3::ZERO, p1), (true, 0.0));
    assert_eq!(plane.compute_intersection(p0, Vector3::ZERO), (true, 1.0));
    assert_eq!(plane.compute_intersection(p1, p1), (false, 0.0));
    let (intersects, t) = plane.compute_intersection(p1, Vector3::new(0.0, 0.000002, 0.0));
    assert!(!intersects);
    assert_eq!(t, -1.0);
    let (intersects, t) = plane.compute_intersection(Vector3::splat(f32::NAN), p1);
    assert!(intersects);
    assert!(t.is_nan());
}
