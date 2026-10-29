use generals_main::game_logic::special_power_strikes::{
    PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES, particle_is_fast_drive,
};

#[test]
fn particle_uplink_double_click_gap_uses_unsigned_frame_wrap() {
    assert_eq!(PARTICLE_DOUBLE_CLICK_FAST_DRIVE_FRAMES, 15);

    // Preserve strict less-than at the finite boundary.
    assert!(particle_is_fast_drive(114, 100));
    assert!(!particle_is_fast_drive(115, 100));
    assert!(!particle_is_fast_drive(131, 100));

    // C++ UnsignedInt subtraction wraps modulo 2^32. These pairs produce
    // gaps 14, 15, and 31 even though the previous click numerically exceeds
    // the current frame.
    let current = 7;
    assert!(particle_is_fast_drive(current, u32::MAX - 6));
    assert!(!particle_is_fast_drive(current, u32::MAX - 7));
    assert!(!particle_is_fast_drive(current, u32::MAX - 23));
}
