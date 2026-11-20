use math_utilities::wwmath::WWMath;
use std::hint::black_box;

fn assert_original<const N: usize>(input: f32, actual: [f32; N], expected: [u32; N]) {
    // The captured C++ reference and Rust both use the host libm for the table.
    // A different libm can round a table entry and its reciprocal differently.
    let max_ulps = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        0
    } else {
        2
    };
    for (actual, expected) in actual.into_iter().zip(expected) {
        let reference = f32::from_bits(expected);
        let actual = actual.to_bits();
        if reference == 0.0 || !reference.is_finite() {
            assert_eq!(actual, expected, "input {input}");
        } else {
            assert!(
                actual.abs_diff(expected) <= max_ulps,
                "input {input}: got {actual:08x}, expected {expected:08x}",
            );
        }
    }
}

// Portable original wwmath.h with the original wwmath.cpp table initialization.
// Include both signed zeros, the first stock water-bank arguments and wraparound.
#[test]
fn periodic_lookup_and_reciprocals_match_cpp() {
    for (bits, expected) in [
        (
            0x8000_0000,
            [0x0000_0000, 0x3f80_0000, 0x7f80_0000, 0x3f80_0000],
        ),
        (
            0x0000_0000,
            [0x0000_0000, 0x3f80_0000, 0x7f80_0000, 0x3f80_0000],
        ),
        (
            0xbbc4_9ba6,
            [0xbbc4_9a0a, 0x3f7f_fecb, 0xc326_ac08, 0x3f80_009b],
        ),
        (
            0xbc44_9ba6,
            [0xbc44_99d1, 0x3f7f_fb3b, 0xc2a6_ac38, 0x3f80_0263],
        ),
        (
            0xbf9e_064b,
            [0xbf71_aa27, 0x3ea8_ec72, 0xbf87_97c5, 0x4041_fb2e],
        ),
        (
            0x3c8b_4396,
            [0x3c8b_41c2, 0x3f7f_f650, 0x426b_4e61, 0x3f80_04d8],
        ),
        (
            0x3f9e_064b,
            [0x3f71_aa27, 0x3ea8_ec71, 0x3f87_97c5, 0x4041_fb2f],
        ),
        (
            0x4049_0fdb,
            [0xb3bb_bd2e, 0xbf80_0000, 0xcb2e_8a4b, 0xbf80_0000],
        ),
        (
            0xc0c8_f6b9,
            [0x3b49_0f88, 0x3f7f_ff62, 0x43a2_f9c6, 0x3f80_004f],
        ),
        (
            0x40c8_f6b9,
            [0xbb49_0e36, 0x3f7f_ff62, 0xc3a2_fad8, 0x3f80_004f],
        ),
        (
            0x4175_f5c3,
            [0x3ea8_8daa, 0xbf71_ba9c, 0x4042_6842, 0xbf87_8e8a],
        ),
        (
            0xc254_f6c9,
            [0xbe29_484e, 0xbf7c_7a34, 0xc0c1_91eb, 0xbf81_c931],
        ),
    ] {
        let angle = black_box(f32::from_bits(bits));
        assert_original(
            angle,
            [
                WWMath::fast_sin(angle),
                WWMath::fast_cos(angle),
                WWMath::fast_inv_sin(angle),
                WWMath::fast_inv_cos(angle),
            ],
            expected,
        );
    }
}

#[test]
fn inverse_lookup_matches_cpp_across_zero_and_fallback() {
    for (bits, expected) in [
        (0xbf80_0000, [0x4049_0fda, 0xbfc9_0fda]),
        (0xbf79_db23, [0x403b_0315, 0xbfac_f64f]),
        (0xbf79_999a, [0x403a_b910, 0xbfac_6245]),
        (0xbebe_b852, [0x3ff9_eb74, 0xbec3_6e65]),
        (0xba83_126f, [0x3fc9_30a0, 0xba83_1274]),
        (0x8000_0000, [0x3fc9_0fdb, 0x0000_0000]),
        (0x3a83_126f, [0x3fc8_ef16, 0x3a83_1274]),
        (0x3ebe_b852, [0x3f98_3441, 0x3ec3_6e65]),
        (0x3f79_999a, [0x3e65_6cae, 0x3fac_6245]),
        (0x3f79_db23, [0x3e60_cc5a, 0x3fac_f64f]),
        (0x3f80_0000, [0x0000_0000, 0x3fc9_0fda]),
    ] {
        let value = black_box(f32::from_bits(bits));
        assert_original(
            value,
            [WWMath::fast_acos(value), WWMath::fast_asin(value)],
            expected,
        );
    }
}
