use math_utilities::{
    Triangle, Vector3, Vector4, cast_semi_infinite_axis_aligned_ray_to_triangle,
    point_in_triangle_2d,
};

#[test]
fn original_tri_h_projected_containment_and_flags() {
    let (a, b, c) = (Vector3::ZERO, Vector3::X, Vector3::Y);
    let almost = 1.0e-8;
    let along = |x, y| Vector3::new(x, y, 0.0);
    // Observations from the compiled portable tri.h. Keep unrelated flag bits.
    let cases = [
        (
            "small winding",
            [a, b, along(0.0, almost)],
            along(0.25, 2.0e-9),
            (true, 0x80),
        ),
        ("almost edge", [a, b, c], along(0.25, almost), (true, 0x80)),
        ("clockwise edge", [a, c, b], along(0.25, 0.0), (true, 0x81)),
        (
            "thin edge miss",
            [a, b, c],
            along(0.25, -almost),
            (false, 0x80),
        ),
        (
            "collinear endpoint",
            [a, b, along(3.0, 0.0)],
            a,
            (false, 0x80),
        ),
        ("collinear center", [a, b, along(3.0, 0.0)], b, (true, 0x81)),
        (
            "collinear almost",
            [a, along(3.0, 0.0), b],
            along(1.0, almost),
            (false, 0x80),
        ),
        (
            "coincident almost",
            [a, a, a],
            along(0.0, almost),
            (false, 0x80),
        ),
        ("coincident exact", [a, a, a], a, (true, 0x81)),
    ];
    for (name, [a, b, c], point, expected) in cases {
        let mut flags = 0x80;
        let hit = point_in_triangle_2d(a, b, c, point, 0, 1, &mut flags);
        assert_eq!((hit, flags), expected, "{name}");
    }
}

#[test]
fn original_tri_h_axis_aligned_ray_and_tri_cpp_projection() {
    let (a, b, c) = (Vector3::ZERO, Vector3::X, Vector3::Y);
    let plane = Vector4::new(0.0, 0.0, 1.0, 0.0);
    let near_edge = Vector3::new(0.25, 0.0, 1.0);
    let inside = Vector3::new(0.25, 0.25, 0.0);
    let cases = [
        (
            "ray into edge",
            plane,
            near_edge,
            [2, 0, 1, 0],
            (true, 0x81),
        ),
        ("ray away", plane, near_edge, [2, 0, 1, 1], (false, 0x80)),
        (
            "ray tiny coefficient",
            plane * 1.0e-8,
            inside,
            [2, 0, 1, 0],
            (true, 0x82),
        ),
        (
            "ray parallel inside",
            plane,
            inside,
            [0, 1, 2, 0],
            (false, 0x82),
        ),
        (
            "ray parallel outside",
            plane,
            Vector3::new(2.0, 0.25, 0.0),
            [0, 1, 2, 0],
            (false, 0x80),
        ),
    ];
    for (name, plane, start, [ray_axis, axis1, axis2, direction], expected) in cases {
        let mut flags = 0x80;
        let hit = cast_semi_infinite_axis_aligned_ray_to_triangle(
            a, b, c, plane, start, ray_axis, axis1, axis2, direction, &mut flags,
        );
        assert_eq!((hit, flags), expected, "{name}");
    }

    let p = Vector3::new(-0.25, 0.25, 0.3);
    let triangle = Triangle::new(a, b, p);
    assert_eq!(
        triangle.normal.to_array().map(f32::to_bits),
        [0, 0xbf44_aa27, 0x3f23_e320],
    );
    assert_eq!(triangle.find_dominant_plane(), (0, 2));
    assert!(triangle.contains_point(p));
    let tied_axes = Triangle::with_normal(a, b, c, Vector3::new(-2.0, 2.0, 2.0));
    assert_eq!(tied_axes.find_dominant_plane(), (1, 2));
}
