use math_utilities::{Sphere, Vector3};

#[test]
fn bounding_sphere_large_coordinates_keep_double_precision_squared_distances() {
    let points = [Vector3::new(-1.0e20, 0.0, 0.0), Vector3::new(1.0e20, 0.0, 0.0)];
    let sphere = Sphere::from_points(&points);
    assert_eq!(sphere.center, Vector3::ZERO);
    assert_eq!(sphere.radius, 1.0e20);
}

#[test]
fn bounding_sphere_expands_for_point_outside_initial_diameter() {
    let points = [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(4.0, 0.0, 0.0),
        Vector3::new(2.0, 3.0, 0.0),
    ];
    let sphere = Sphere::from_points(&points);
    assert_eq!(sphere.center, Vector3::new(2.0, 0.5, 0.0));
    assert_eq!(sphere.radius, 2.5);
}

#[test]
fn coincident_spheres_follow_cpp_conditional_for_nan_radii() {
    let mut sphere = Sphere::new(Vector3::ZERO, 2.0);
    sphere.add_sphere(&Sphere::new(Vector3::ZERO, f32::NAN));
    assert!(sphere.radius.is_nan());
}

#[test]
fn tangent_spheres_do_not_intersect_and_zero_radius_addition_is_asymmetric() {
    let sphere = Sphere::new(Vector3::ZERO, 1.0);
    let tangent = Sphere::new(Vector3::new(2.0, 0.0, 0.0), 1.0);
    assert!(!sphere.intersects(&tangent));
    let point = Sphere::new(Vector3::new(10.0, 0.0, 0.0), 0.0);
    assert_eq!(sphere.clone() + point.clone(), sphere);
    assert_eq!(point + sphere.clone(), sphere);
}
