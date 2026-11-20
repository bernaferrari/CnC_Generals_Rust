use math_utilities::{AABox, LineSegment, Matrix3D, MinMaxAABox, Vector3, WWMath, WwMathRng};
use std::mem::{align_of, offset_of, size_of};

fn bits(first: Vector3, second: Vector3) -> [u32; 6] {
    [first.x, first.y, first.z, second.x, second.y, second.z].map(f32::to_bits)
}

#[test]
fn original_box_layout_and_ordered_bounds_keep_exact_bits() {
    assert_eq!(size_of::<AABox>(), 24);
    assert_eq!(align_of::<AABox>(), 4);
    assert_eq!(offset_of!(AABox, center), 0);
    assert_eq!(offset_of!(AABox, extent), 12);
    assert_eq!(size_of::<MinMaxAABox>(), 24);
    assert_eq!(align_of::<MinMaxAABox>(), 4);
    assert_eq!(offset_of!(MinMaxAABox, min_corner), 0);
    assert_eq!(offset_of!(MinMaxAABox, max_corner), 12);

    let points = [
        Vector3::new(-0.0, f32::from_bits(0x7fc01234), 1.5),
        Vector3::new(0.0, 2.0, -3.25),
        Vector3::new(0.0, -1.0, 4.75),
    ];
    let point_box = AABox::from_points(&points);
    let [cx, cy, cz, ex, ey, ez] = bits(point_box.center, point_box.extent);
    assert_eq!([cx, cz, ex, ez], [0x80000000, 0x3f400000, 0, 0x40800000]);
    assert!(f32::from_bits(cy).is_nan() && f32::from_bits(ey).is_nan());
    let mut minmax = MinMaxAABox::from_points(&points);
    assert_eq!(
        bits(minmax.min_corner, minmax.max_corner),
        [
            0x80000000, 0x7fc01234, 0xc0500000, 0x80000000, 0x7fc01234, 0x40980000
        ]
    );
    minmax.add_point(Vector3::new(0.0, 12.0, -5.125));
    assert_eq!(
        bits(minmax.min_corner, minmax.max_corner),
        [
            0x80000000, 0x7fc01234, 0xc0a40000, 0x80000000, 0x7fc01234, 0x40980000
        ]
    );
    minmax.init_empty();
    assert_eq!(
        bits(minmax.min_corner, minmax.max_corner),
        [
            0x7f7fffff, 0x7f7fffff, 0x7f7fffff, 0xff7fffff, 0xff7fffff, 0xff7fffff
        ]
    );
}

#[test]
fn original_line_addition_and_affine_transforms_keep_exact_bits() {
    let line = LineSegment::new(
        Vector3::new(5.5, -1.25, 8.0),
        Vector3::new(-4.5, 16.25, 8.0),
    );
    let mut bounds = AABox::ZERO;
    bounds.init_from_line_segment(&line);
    assert_eq!(
        bits(bounds.center, bounds.extent),
        [
            0x3f000000, 0x40f00000, 0x41000000, 0x40a00000, 0x410c0000, 0
        ]
    );
    bounds.add_point(Vector3::new(-12.0, 13.25, -1.25));
    bounds.add_box(&AABox::new(
        Vector3::new(12.0, 12.25, 19.5),
        Vector3::new(0.25, 1.5, 0.0),
    ));
    assert_eq!(
        bits(bounds.center, bounds.extent),
        [
            0x3e000000, 0x40f00000, 0x41120000, 0x41420000, 0x410c0000, 0x41260000
        ]
    );
    assert_eq!(bounds.volume().to_bits(), 0x46099720);
    assert_eq!(
        bounds
            .project_to_axis(Vector3::new(-0.5, 1.25, 0.75))
            .to_bits(),
        0x41c64000
    );
    let matrix = Matrix3D::from_array(&[
        1.0, 1.0, 0.0, 1.0, 0.0, -0.5, 2.0, -4.0, 0.25, 0.0, -1.0, 7.125,
    ]);
    let mut minmax = MinMaxAABox::from_aabox(&bounds);
    bounds.transform(&matrix);
    minmax.transform(&matrix);
    assert_eq!(
        bits(bounds.center, bounds.extent),
        [
            0x410a0000, 0x41280000, 0xbffc0000, 0x41a70000, 0x41c90000, 0x41568000
        ]
    );
    assert_eq!(
        bits(minmax.min_corner, minmax.max_corner),
        [
            0xc1440000, 0xc16a0000, 0xc1760000, 0x41ec0000, 0x420e8000, 0x41370000
        ]
    );
}

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
    let matrix = Matrix3D::from_values(1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
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
