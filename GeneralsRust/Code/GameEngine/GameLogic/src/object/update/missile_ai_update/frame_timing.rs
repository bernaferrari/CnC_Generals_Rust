//! MissileAIUpdate.cpp unsigned frame expressions, evaluated on the driving
//! owner's observations. No clock publication or registry access is needed.

pub(super) fn ignition_ready(now: u32, entered: u32, delay: u32) -> bool {
    // doLaunchState: now - m_stateTimestamp >= delay.
    now.wrapping_sub(entered) >= delay
}

pub(super) fn fuel_expiration(now: u32, lifetime: u32) -> u32 {
    // doIgnitionState uses the signed maximum as its historical sentinel.
    if lifetime == 0 {
        0x7fff_ffff
    } else {
        now.wrapping_add(lifetime)
    }
}

pub(super) fn hold_for_contrail(now: u32, entered: u32, delay: u32) -> bool {
    // Preserve the original expression, including its near-zero behavior;
    // rewriting this as elapsed >= delay changes unsigned boundary results.
    entered > now.wrapping_sub(delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignition_waits_through_frame_wrap() {
        let entered = u32::MAX - 1;
        assert!(!ignition_ready(u32::MAX, entered, 3));
        assert!(!ignition_ready(0, entered, 3));
        assert!(ignition_ready(1, entered, 3));
        assert!(ignition_ready(entered, entered, 0));
    }

    #[test]
    fn fuel_uses_original_sentinel_and_wrapping_deadline() {
        assert_eq!(fuel_expiration(123, 0), 0x7fff_ffff);
        assert_eq!(fuel_expiration(u32::MAX - 1, 3), 1);
        assert_eq!(fuel_expiration(30, 150), 180);
    }

    #[test]
    fn contrail_hold_preserves_original_unsigned_expression() {
        assert!(hold_for_contrail(101, 100, 3));
        assert!(hold_for_contrail(102, 100, 3));
        assert!(!hold_for_contrail(103, 100, 3));
        assert!(!hold_for_contrail(0, 0, 3));
        assert!(hold_for_contrail(0, u32::MAX - 1, 3));
        assert!(!hold_for_contrail(1, u32::MAX - 1, 3));
        assert!(!hold_for_contrail(2, u32::MAX - 1, 3));
    }
}
