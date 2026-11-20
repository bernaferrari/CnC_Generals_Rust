use math_utilities::{CollisionMath, EPSILON, OverlapType, Plane, Sphere, Vector3};

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

#[test]
fn geometry_and_collision_share_the_original_plane_distance() {
    let mut plane = Plane::new(Vector3::X, 0.0);
    let point = Vector3::new(3.0, 0.0, 0.0);
    assert!(plane.is_point_in_front(point));
    assert_eq!(
        CollisionMath::overlap_test_plane_point(&plane, &point),
        OverlapType::Positive,
    );

    // Writing PlaneClass::D changes both the point and collision predicates.
    plane.distance = 5.0;
    assert!(!plane.is_point_in_front(point));
    assert_eq!(
        CollisionMath::overlap_test_plane_point(&plane, &point),
        OverlapType::Negative,
    );
    assert!(plane.is_sphere_in_front(&Sphere::new(Vector3::new(6.0, 0.0, 0.0), 1.0)));
    assert!(!plane.is_sphere_in_front_or_intersecting(&Sphere::new(Vector3::new(4.0, 0.0, 0.0), 1.0)));
}

#[test]
fn intersect_planes_uses_the_original_largest_component_branches() {
    let x = Plane::new(Vector3::X, 5.0);
    let y = Plane::new(Vector3::Y, -2.0);
    let z = Plane::new(Vector3::Z, 7.0);
    for (a, b, direction, point) in [
        (y, z, Vector3::X, Vector3::new(0.0, -2.0, 7.0)),
        (z, x, Vector3::Y, Vector3::new(5.0, 0.0, 7.0)),
        (x, y, Vector3::Z, Vector3::new(5.0, -2.0, 0.0)),
    ] {
        assert_eq!(a.intersect_with_plane(&b), (direction, point));
    }
}
