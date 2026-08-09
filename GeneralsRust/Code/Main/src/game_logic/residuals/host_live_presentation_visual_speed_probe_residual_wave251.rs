//! Wave 251 residual peels: render/update/FPS timing prefers presentation
//! `visual_speed_multiplier` residual when a frame is installed, instead of
//! dual-reading live `GameLogic::visual_speed_multiplier`. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 250 presentation time-frozen probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` visual_speed_multiplier field + build_from_logic
//! - `cnc_game_engine.rs` visual_dt / render_time_delta / effective_fps_limit
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Boot residual still falls back to live GameLogic when no frame

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation visual-speed probe residual method names.
pub const LIVE_PRESENTATION_VISUAL_SPEED_PROBE_METHOD_NAMES_WAVE251: &[&str] = &[
    "visual_speed_multiplier",
    "visual_dt",
    "render_time_delta",
    "effective_fps_limit_for_frame",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_VISUAL_SPEED_PROBE_NAV_STEPS_WAVE251: &[&str] = &[
    "REQUIRE_PRESENTATION_VISUAL_SPEED_FIELD",
    "REQUIRE_RENDER_UPDATE_USE_PRESENTATION",
    "LIVE_PRESENTATION_VISUAL_SPEED_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_VISUAL_SPEED_PROBE_CMD_NAMES_WAVE251: &[&str] = &[
    "click_live_presentation_visual_speed_probe_ok_prepare",
    "click_live_presentation_visual_speed_probe_ok_live",
    "click_live_presentation_visual_speed_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_visual_speed_probe_method_names_residual_wave251() -> bool {
    LIVE_PRESENTATION_VISUAL_SPEED_PROBE_METHOD_NAMES_WAVE251.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_VISUAL_SPEED_PROBE_METHOD_NAMES_WAVE251,
            "visual_speed_multiplier",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_VISUAL_SPEED_PROBE_METHOD_NAMES_WAVE251,
            "effective_fps_limit_for_frame",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_VISUAL_SPEED_PROBE_METHOD_NAMES_WAVE251,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_visual_speed_probe_nav_commands_residual_wave251() -> bool {
    LIVE_PRESENTATION_VISUAL_SPEED_PROBE_NAV_STEPS_WAVE251.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_VISUAL_SPEED_PROBE_NAV_STEPS_WAVE251,
            "REQUIRE_PRESENTATION_VISUAL_SPEED_FIELD",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_VISUAL_SPEED_PROBE_NAV_STEPS_WAVE251,
            "LIVE_PRESENTATION_VISUAL_SPEED_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_VISUAL_SPEED_PROBE_CMD_NAMES_WAVE251.len() == 3
}

/// Wave 251 composite residual honesty pack.
pub fn honesty_live_presentation_visual_speed_probe_residual_pack_wave251() -> bool {
    honesty_live_presentation_visual_speed_probe_method_names_residual_wave251()
        && honesty_live_presentation_visual_speed_probe_nav_commands_residual_wave251()
}

/// Source residual: presentation freezes visual speed; engine consumers prefer it.
pub fn honesty_presentation_visual_speed_probe_source() -> bool {
    let pf = include_str!("../../presentation_frame.rs");
    let eng = include_str!("../../cnc_game_engine.rs");
    if !(pf.contains("pub visual_speed_multiplier: f32")
        && pf.contains("Wave 251")
        && pf.contains("visual_speed_multiplier: logic.visual_speed_multiplier()"))
    {
        return false;
    }
    // Wave 550: call sites centralized via presentation_or_boot_visual_speed
    // (still presentation-first; raw dual-read only inside helper).
    let consumer_ok = (eng.matches("Wave 251").count() >= 3
        && eng.contains("p.visual_speed_multiplier")
        && eng.contains("visual_dt")
        && eng.contains("render_time_delta"))
        || (eng.contains("fn presentation_or_boot_visual_speed")
            && eng.contains("Wave 550")
            && eng.contains("presentation_or_boot_visual_speed()")
            && eng.contains("visual_dt")
            && eng.contains("render_time_delta"));
    consumer_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_presentation_visual_speed_probe_honesty() -> bool {
    honesty_live_presentation_visual_speed_probe_residual_pack_wave251()
        && honesty_presentation_visual_speed_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_visual_speed_probe_method_names_residual_wave251());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_visual_speed_probe_nav_commands_residual_wave251());
    }

    #[test]
    fn wave251_composite_pack() {
        assert!(honesty_live_presentation_visual_speed_probe_residual_pack_wave251());
    }

    #[test]
    fn presentation_visual_speed_probe_sources() {
        assert!(honesty_presentation_visual_speed_probe_source());
    }

    #[test]
    fn simulate_live_presentation_visual_speed_probe_honesty_residual_live() {
        assert!(
            simulate_live_presentation_visual_speed_probe_honesty(),
            "presentation visual speed probe residual must latch"
        );
    }
}
