use math_utilities::{CastResultStruct, Vector3, raycast_flags};
use std::mem::{align_of, offset_of, size_of};

#[test]
fn cast_result_matches_original_reset_and_32_bit_layout() {
    let expected = CastResultStruct {
        start_bad: false,
        fraction: 1.0,
        normal: Vector3::ZERO,
        surface_type: 0,
        compute_contact_point: false,
        contact_point: Vector3::ZERO,
    };
    assert_eq!(CastResultStruct::new(), expected);
    let mut reused = CastResultStruct {
        start_bad: true,
        fraction: 0.25,
        normal: Vector3::new(-1.0, 2.0, -3.0),
        surface_type: u32::MAX,
        compute_contact_point: true,
        contact_point: Vector3::new(4.0, -5.0, 6.0),
    };
    reused.reset();
    assert_eq!(reused, expected);

    assert_eq!(size_of::<CastResultStruct>(), 40);
    assert_eq!(align_of::<CastResultStruct>(), 4);
    assert_eq!(offset_of!(CastResultStruct, start_bad), 0);
    assert_eq!(offset_of!(CastResultStruct, fraction), 4);
    assert_eq!(offset_of!(CastResultStruct, normal), 8);
    assert_eq!(offset_of!(CastResultStruct, surface_type), 20);
    assert_eq!(offset_of!(CastResultStruct, compute_contact_point), 24);
    assert_eq!(offset_of!(CastResultStruct, contact_point), 28);
    assert_eq!(raycast_flags::NONE, 0);
    assert_eq!(raycast_flags::HIT_EDGE, 1);
    assert_eq!(raycast_flags::START_IN_TRI, 2);
}
