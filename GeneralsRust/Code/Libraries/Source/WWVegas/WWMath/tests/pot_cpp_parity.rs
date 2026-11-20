use math_utilities::{find_pot, find_pot_log2, is_power_of_2, next_power_of_2, prev_power_of_2};

#[test]
fn power_of_two_rounding_matches_original_executable() {
    let mut digest = 14_695_981_039_346_656_037u64;
    let mut record = |value| {
        digest ^= u64::from(value);
        digest = digest.wrapping_mul(1_099_511_628_211);
    };
    let boundaries = [
        0x3fff_ffff,
        0x4000_0000,
        0x4000_0001,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        0xffff_fffe,
        0xffff_ffff,
    ];
    for value in (0..=65_536).chain(boundaries) {
        if let Ok(signed) = i32::try_from(value) {
            record(find_pot(signed) as u32);
        }
        record(find_pot_log2(value));
    }
    // Executed against the original WWMath/pot.cpp in this input order.
    assert_eq!(digest, 0x97cf_f49c_73f1_a27b);
}

#[test]
fn safe_rust_extensions_keep_existing_boundary_results() {
    assert_eq!(find_pot(i32::MIN), 1);
    assert_eq!(find_pot(-1), 1);
    assert_eq!(next_power_of_2(0), 1);
    assert_eq!(next_power_of_2(0x4000_0000), 0x8000_0000);
    assert_eq!(next_power_of_2(0x8000_0000), 1);
    assert_eq!(next_power_of_2(u32::MAX), 1);
    assert_eq!(prev_power_of_2(0), 0);
    assert_eq!(prev_power_of_2(u32::MAX), 0x8000_0000);
    assert!(is_power_of_2(0x8000_0000));
    assert!(!is_power_of_2(u32::MAX));
}
