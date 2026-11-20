//! Bit patterns from the original portable `vector4.h` using nearest rounding
//! and contraction disabled.

use math_utilities::{Vector4, vector4::Vector4Ext};

fn bits(vector: Vector4) -> [u32; 4] {
    vector.to_array().map(f32::to_bits)
}

#[test]
fn portable_normalization_keeps_original_accumulation_and_zero_edges() {
    let small = 1.0 / 4096.0;
    let samples = [
        (
            Vector4::new(0.0, 3.0, -0.0, 0.0),
            [0, 0x3f80_0000, 0x8000_0000, 0],
        ),
        (
            Vector4::new(-1.125, 7.25, 4.375, -3.0625),
            [0xbdfd_e586, 0x3f4c_871e, 0x3ef6_d80a, 0xbeac_ca6d],
        ),
        (
            Vector4::new(1.0, small, small, small),
            [0x3f80_0000, 0x3980_0000, 0x3980_0000, 0x3980_0000],
        ),
    ];
    for (mut vector, expected) in samples {
        assert_eq!(bits(vector.normalized_or_zero()), expected);
        vector.normalize_in_place();
        assert_eq!(bits(vector), expected);
    }

    for (mut vector, expected) in [
        (
            Vector4::new(-0.0, 0.0, -0.0, 0.0),
            [0x8000_0000, 0, 0x8000_0000, 0],
        ),
        (
            Vector4::new(-1e-30, 0.0, 0.0, 2e-30),
            [0x8da2_4260, 0, 0, 0x0e22_4260],
        ),
    ] {
        assert_eq!(bits(vector.normalized_or_zero()), [0; 4]);
        vector.normalize_in_place();
        assert_eq!(bits(vector), expected);
    }
}

#[test]
fn interpolation_uses_all_components_of_the_original_value_result() {
    let mut a = Vector4::new(-5.0625, 3.0, 11.0, -2.125);
    let mut b = Vector4::new(2.9375, -16.5, 11.0, 1.875);
    let mut result = Vector4::splat(91.0);
    Vector4::lerp_into(&mut result, a, b, 0.3725);
    assert_eq!(
        bits(result),
        [0xc005_47ae, 0xc088_70a4, 0x4130_0000, 0xbf22_8f5c]
    );
    Vector4::lerp_into(&mut result, a, b, -1.25);
    assert_eq!(
        bits(result),
        [0xc171_0000, 0x41db_0000, 0x4130_0000, 0xc0e4_0000]
    );
    Vector4::swap(&mut a, &mut b);
    assert_eq!(
        bits(a),
        [0x403c_0000, 0xc184_0000, 0x4130_0000, 0x3ff0_0000]
    );
    assert_eq!(
        bits(b),
        [0xc0a2_0000, 0x4040_0000, 0x4130_0000, 0xc008_0000]
    );
}

#[test]
fn set_copy_and_validity_use_original_component_order() {
    let mut vector = Vector4::ZERO;
    vector.set(-0.0, -3.125, 7.25, 1.5);
    assert_eq!(
        bits(vector),
        [0x8000_0000, 0xc048_0000, 0x40e8_0000, 0x3fc0_0000]
    );
    assert!(vector.is_valid());
    let mut copy = Vector4::ONE;
    copy.set_from(vector);
    assert_eq!(bits(copy), bits(vector));
    assert!(!Vector4::new(0.0, 1.0, -2.0, f32::INFINITY).is_valid());
    assert!(!Vector4::new(f32::NAN, 1.0, 2.0, 3.0).is_valid());
    assert!(Vector4::new(f32::from_bits(1), 0.0, 0.0, 0.0).is_valid());
}
