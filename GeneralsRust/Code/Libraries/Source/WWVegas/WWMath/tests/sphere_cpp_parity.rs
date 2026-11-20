use math_utilities::{Matrix3D, Sphere, Vector3, add_spheres, transform_sphere};
use std::mem::{align_of, offset_of, size_of};

fn bits(sphere: &Sphere) -> [u32; 4] {
    [
        sphere.center.x,
        sphere.center.y,
        sphere.center.z,
        sphere.radius,
    ]
    .map(f32::to_bits)
}

#[test]
fn original_sphere_layout_expand_recenter_and_transform_keep_exact_bits() {
    assert_eq!(size_of::<Sphere>(), 16);
    assert_eq!(align_of::<Sphere>(), 4);
    assert_eq!(offset_of!(Sphere, center), 0);
    assert_eq!(offset_of!(Sphere, radius), 12);

    let points = [
        Vector3::new(-5.25, 4.125, 0.5),
        Vector3::new(3.5, -4.75, -1.5),
        Vector3::new(2.25, 3.75, 6.0),
        Vector3::new(1.0, -4.0, 2.5),
    ];
    let sphere = Sphere::from_points(&points);
    assert_eq!(
        bits(&sphere),
        [0xbf00fa76, 0x3e2e1cce, 0x3e8b4a3f, 0x40e96c32]
    );
    assert_eq!(sphere.volume().to_bits(), 0x44cb39a9);

    let mut moved = Sphere::from_center_and_sphere(Vector3::new(3.125, -0.5, 5.25), &sphere);
    assert_eq!(
        bits(&moved),
        [0x40480000, 0xbf000000, 0x40a80000, 0x4157db2e]
    );
    moved.re_center(Vector3::new(-1.875, 2.5, -2.625));
    assert_eq!(
        bits(&moved),
        [0xbff00000, 0x40200000, 0xc0280000, 0x41ba5172]
    );

    let matrix = Matrix3D::from_array(&[
        0.0, -1.0, 0.0, 12.25, 1.0, 0.0, 0.0, -10.0, 0.0, 0.0, 1.0, 6.625,
    ]);
    let transformed = transform_sphere(&matrix, &moved);
    assert_eq!(
        bits(&transformed),
        [0x411c0000, 0xc13e0000, 0x40800000, 0x41ba5172]
    );
    moved *= &matrix;
    assert_eq!(moved, transformed);

    let contained = Sphere::new(Vector3::new(10.5, -11.5, 4.0), 0.8);
    let crossing = Sphere::new(Vector3::new(-11.5, 0.25, 8.0), 6.0);
    assert!(moved.intersects(&contained));
    assert!(moved.intersects(&crossing));
    moved += contained;
    assert_eq!(moved, transformed);
    moved += crossing;
    assert_eq!(
        bits(&moved),
        [0x40d12051, 0xc120a696, 0x40935d4e, 0x41d85262]
    );
    let larger = Sphere::new(Vector3::new(1.0, 2.0, 3.0), 75.3);
    assert_eq!(add_spheres(&moved, &larger), larger);
    assert_eq!(add_spheres(&larger, &moved), larger);
}

#[test]
fn bounding_sphere_large_coordinates_keep_double_precision_squared_distances() {
    let points = [
        Vector3::new(-1.0e20, 0.0, 0.0),
        Vector3::new(1.0e20, 0.0, 0.0),
    ];
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
