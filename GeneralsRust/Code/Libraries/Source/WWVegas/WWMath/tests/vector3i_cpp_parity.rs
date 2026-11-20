use math_utilities::{Vector3i, Vector3i16};

#[test]
fn original_three_component_vectors_preserve_signed_and_unsigned_comparisons() {
    let mut wide = Vector3i::new(i32::MIN, 0, i32::MAX);
    let same = Vector3i::new(i32::MIN, 0, i32::MAX);
    assert_eq!(wide, same);
    wide[1] = -57;
    assert_ne!(wide, same);
    assert_eq!(wide.to_array(), [i32::MIN, -57, i32::MAX]);

    let mut narrow = Vector3i16::new(65_535, 32_768, 0);
    let other = Vector3i::new(65_535, 32_768, 0);
    assert_ne!(narrow, Vector3i::new(-1, 32_768, 0));
    assert_eq!(narrow, other);
    narrow[2] = 17;
    assert_ne!(narrow, other);
    assert_eq!(narrow.to_array(), [65_535, 32_768, 17]);

    assert_eq!(std::mem::size_of::<Vector3i>(), 12);
    assert_eq!(std::mem::offset_of!(Vector3i, x), 0);
    assert_eq!(std::mem::offset_of!(Vector3i, y), 4);
    assert_eq!(std::mem::offset_of!(Vector3i, z), 8);
    assert_eq!(std::mem::size_of::<Vector3i16>(), 6);
    assert_eq!(std::mem::offset_of!(Vector3i16, i), 0);
    assert_eq!(std::mem::offset_of!(Vector3i16, j), 2);
    assert_eq!(std::mem::offset_of!(Vector3i16, k), 4);
}
