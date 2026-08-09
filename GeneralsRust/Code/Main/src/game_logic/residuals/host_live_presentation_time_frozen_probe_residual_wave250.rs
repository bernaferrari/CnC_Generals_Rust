//! Wave 250 residual peels: render/update visual timing prefers presentation
//! `time_frozen_for_simulation` freeze residual when a frame is installed,
//! instead of dual-reading live `GameLogic::is_time_frozen_for_simulation`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 249 client dual-world empty-gate residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` render_time_delta / visual_delta / shake_dt /
//!   update_internal freeze gates
//! - `presentation_frame.rs` time_frozen_for_simulation
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - visual_speed_multiplier still live residual when not frozen

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation time-frozen probe residual method names.
pub const LIVE_PRESENTATION_TIME_FROZEN_PROBE_METHOD_NAMES_WAVE250: &[&str] = &[
    "time_frozen_for_simulation",
    "render_time_delta",
    "visual_delta",
    "shake_dt",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_TIME_FROZEN_PROBE_NAV_STEPS_WAVE250: &[&str] = &[
    "REQUIRE_PRESENTATION_TIME_FROZEN_FIELD",
    "REQUIRE_RENDER_UPDATE_USE_PRESENTATION",
    "LIVE_PRESENTATION_TIME_FROZEN_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_TIME_FROZEN_PROBE_CMD_NAMES_WAVE250: &[&str] = &[
    "click_live_presentation_time_frozen_probe_ok_prepare",
    "click_live_presentation_time_frozen_probe_ok_live",
    "click_live_presentation_time_frozen_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_time_frozen_probe_method_names_residual_wave250() -> bool {
    LIVE_PRESENTATION_TIME_FROZEN_PROBE_METHOD_NAMES_WAVE250.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_TIME_FROZEN_PROBE_METHOD_NAMES_WAVE250,
            "time_frozen_for_simulation",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_TIME_FROZEN_PROBE_METHOD_NAMES_WAVE250,
            "shake_dt",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_TIME_FROZEN_PROBE_METHOD_NAMES_WAVE250,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_time_frozen_probe_nav_commands_residual_wave250() -> bool {
    LIVE_PRESENTATION_TIME_FROZEN_PROBE_NAV_STEPS_WAVE250.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_TIME_FROZEN_PROBE_NAV_STEPS_WAVE250,
            "REQUIRE_PRESENTATION_TIME_FROZEN_FIELD",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_TIME_FROZEN_PROBE_NAV_STEPS_WAVE250,
            "LIVE_PRESENTATION_TIME_FROZEN_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_TIME_FROZEN_PROBE_CMD_NAMES_WAVE250.len() == 3
}

/// Wave 250 composite residual honesty pack.
pub fn honesty_live_presentation_time_frozen_probe_residual_pack_wave250() -> bool {
    honesty_live_presentation_time_frozen_probe_method_names_residual_wave250()
        && honesty_live_presentation_time_frozen_probe_nav_commands_residual_wave250()
}

/// Source residual: engine timing prefers presentation freeze residual.
pub fn honesty_presentation_time_frozen_probe_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let pf = include_str!("../../presentation_frame.rs");
    if !pf.contains("pub time_frozen_for_simulation: bool") {
        return false;
    }
    // Wave 551: call sites centralized via presentation_or_boot_time_frozen
    // (still presentation-first; raw dual-read only inside helper).
    let consumer_ok = (eng.matches("Wave 250").count() >= 3
        && eng.contains("p.time_frozen_for_simulation")
        && eng.contains("render_time_delta")
        && eng.contains("visual_delta")
        && eng.contains("shake_dt"))
        || (eng.contains("fn presentation_or_boot_time_frozen")
            && eng.contains("Wave 551")
            && eng.contains("presentation_or_boot_time_frozen()")
            && eng.contains("render_time_delta")
            && eng.contains("visual_delta")
            && eng.contains("shake_dt"));
    consumer_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_presentation_time_frozen_probe_honesty() -> bool {
    honesty_live_presentation_time_frozen_probe_residual_pack_wave250()
        && honesty_presentation_time_frozen_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_time_frozen_probe_method_names_residual_wave250());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_time_frozen_probe_nav_commands_residual_wave250());
    }

    #[test]
    fn wave250_composite_pack() {
        assert!(honesty_live_presentation_time_frozen_probe_residual_pack_wave250());
    }

    #[test]
    fn presentation_time_frozen_probe_sources() {
        assert!(honesty_presentation_time_frozen_probe_source());
    }

    #[test]
    fn simulate_live_presentation_time_frozen_probe_honesty_residual_live() {
        assert!(
            simulate_live_presentation_time_frozen_probe_honesty(),
            "presentation time frozen probe residual must latch"
        );
    }
}
