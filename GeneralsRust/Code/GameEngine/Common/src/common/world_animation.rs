//! Immutable transport from the receiving world's presentation owner.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimationPresentation {
    /// Monotonic identity within a receiving host epoch; not reused on clear.
    pub identity: u64,
    pub template: String,
    pub position: [f32; 3],
    pub duration: f32,
    pub rise: f32,
    pub fades: bool,
    pub spawn_frame: u32,
    /// Captured once at birth, including client-visual randomization.
    pub initial_frame: u16,
}

/// C++ InGameUI::addWorldAnimation converts the frame to Real, adds the
/// rounded Real duration product, then converts the sum to UnsignedInt.
/// Reject inputs outside the defined unsigned conversion domain.
pub fn expiry_frame(spawn_frame: u32, duration: f32) -> Option<u32> {
    if !duration.is_finite() || duration <= 0.0 {
        return None;
    }
    let duration_frames = duration * 30.0_f32;
    let expire = spawn_frame as f32 + duration_frames;
    if !expire.is_finite() || !(0.0..4_294_967_296.0_f32).contains(&expire) {
        return None;
    }
    Some(expire as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_matches_executed_cpp_real_addition_and_unsigned_conversion() {
        for (frame, duration, expected) in [
            (0, 1.0, 30),
            (0, 0.01, 0),
            (7, 0.05, 8),
            (16_777_216, 0.05, 16_777_218),
            (16_777_217, 0.05, 16_777_218),
            (100_000_001, 1.0, 100_000_032),
            (4_294_967_040, 0.01, 4_294_967_040),
        ] {
            assert_eq!(expiry_frame(frame, duration), Some(expected));
        }
    }
    #[test]
    fn undefined_original_unsigned_conversion_inputs_are_rejected() {
        for duration in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            assert_eq!(expiry_frame(0, duration), None);
        }
        assert_eq!(expiry_frame(u32::MAX, 1.0), None);
    }
}
