use math_utilities::{AAPlane, AxisEnum, Vector3};

#[test]
fn original_constructor_and_set_keep_distance_bits_and_replace_normal() {
    // Portable execution of aaplane.h with dirty Vector3 destinations.
    let mut plane = AAPlane::new(AxisEnum::XNormal, -0.0);
    let rows = [
        (AxisEnum::XNormal, 0x8000_0000, [1.0_f32, 0.0, 0.0]),
        (AxisEnum::YNormal, 0x7fc0_1234, [0.0, 1.0, 0.0]),
        (AxisEnum::ZNormal, 0xc0a8_0000, [0.0, 0.0, 1.0]),
    ];
    assert_eq!(plane.normal, rows[0].0);
    assert_eq!(plane.dist.to_bits(), rows[0].1);
    for (ordinal, (axis, dist, expected)) in rows.into_iter().enumerate() {
        let mut destination = Vector3::new(-0.0, f32::NAN, -7.0);
        plane.set(axis, f32::from_bits(dist));
        plane.get_normal_into(&mut destination);
        assert_eq!(usize::from(plane.normal), ordinal);
        assert_eq!(plane.dist.to_bits(), dist);
        assert_eq!(
            destination.to_array().map(f32::to_bits),
            expected.map(f32::to_bits),
        );
    }
}
