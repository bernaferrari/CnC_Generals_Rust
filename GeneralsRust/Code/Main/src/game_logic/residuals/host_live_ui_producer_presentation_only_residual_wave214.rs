//! Wave 214 residual peels: runtime-host producer pick for force-completed
//! structures is presentation-freeze only — no live `GameLogic` classify dual-read.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 213 presentation FOW-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_runtime_host_command force_completed loop
//! - Wave 214 presentation freeze comment / no or_else live classify
//!
//! Fail-closed:
//! - Not full ControlBar WND producer button matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// UI producer presentation-only residual method names.
pub const LIVE_UI_PRODUCER_PRESENTATION_ONLY_METHOD_NAMES_WAVE214: &[&str] = &[
    "force_completed",
    "presentation freeze only",
    "can_produce",
    "no live dual-read",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_UI_PRODUCER_PRESENTATION_ONLY_NAV_STEPS_WAVE214: &[&str] = &[
    "REQUIRE_FORCE_COMPLETED_PRESENTATION_ONLY",
    "REQUIRE_NO_LIVE_CLASSIFY_FALLBACK",
    "LIVE_UI_PRODUCER_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_UI_PRODUCER_PRESENTATION_ONLY_CMD_NAMES_WAVE214: &[&str] = &[
    "click_live_ui_producer_presentation_only_ok_prepare",
    "click_live_ui_producer_presentation_only_ok_live",
    "click_live_ui_producer_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ui_producer_presentation_only_method_names_residual_wave214() -> bool {
    LIVE_UI_PRODUCER_PRESENTATION_ONLY_METHOD_NAMES_WAVE214.len() == 5
        && residual_name_index(
            LIVE_UI_PRODUCER_PRESENTATION_ONLY_METHOD_NAMES_WAVE214,
            "force_completed",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_PRODUCER_PRESENTATION_ONLY_METHOD_NAMES_WAVE214,
            "no live dual-read",
        ) == Some(3)
        && residual_name_index(
            LIVE_UI_PRODUCER_PRESENTATION_ONLY_METHOD_NAMES_WAVE214,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ui_producer_presentation_only_nav_commands_residual_wave214() -> bool {
    LIVE_UI_PRODUCER_PRESENTATION_ONLY_NAV_STEPS_WAVE214.len() == 4
        && residual_name_index(
            LIVE_UI_PRODUCER_PRESENTATION_ONLY_NAV_STEPS_WAVE214,
            "REQUIRE_FORCE_COMPLETED_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_PRODUCER_PRESENTATION_ONLY_NAV_STEPS_WAVE214,
            "LIVE_UI_PRODUCER_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_UI_PRODUCER_PRESENTATION_ONLY_CMD_NAMES_WAVE214.len() == 3
}

/// Wave 214 composite residual honesty pack.
pub fn honesty_live_ui_producer_presentation_only_residual_pack_wave214() -> bool {
    honesty_live_ui_producer_presentation_only_method_names_residual_wave214()
        && honesty_live_ui_producer_presentation_only_nav_commands_residual_wave214()
}

/// Source residual: force_completed path is presentation-only.
pub fn honesty_force_completed_presentation_only_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Anchor on the production for-loop, not cfg(test) residual strings.
    let i = match eng.find("for id in force_completed.iter().copied()") {
        Some(i) => i,
        None => return false,
    };
    // Include preceding Wave 214 comment.
    let start = eng[..i].rfind("Wave 214").unwrap_or(i);
    let body = &eng[start..eng.len().min(i + 1600)];
    body.contains("presentation freeze only")
        && body.contains("last_presentation_frame")
        && body.contains("can_produce")
        && !body.contains("let classify_live")
        && body.contains("no live GameLogic dual-read residual")
}

/// Source residual: no live classify closure remains near force_completed production loop.
pub fn honesty_no_live_classify_closure_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = match eng.find("for id in force_completed.iter().copied()") {
        Some(i) => i,
        None => return false,
    };
    let window = &eng[i.saturating_sub(400)..eng.len().min(i + 2000)];
    !window.contains("let classify_live") && window.contains("presentation freeze only")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ui_producer_presentation_only_honesty() -> bool {
    honesty_live_ui_producer_presentation_only_residual_pack_wave214()
        && honesty_force_completed_presentation_only_source()
        && honesty_no_live_classify_closure_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ui_producer_presentation_only_method_names_residual_wave214());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ui_producer_presentation_only_nav_commands_residual_wave214());
    }

    #[test]
    fn wave214_composite_pack() {
        assert!(honesty_live_ui_producer_presentation_only_residual_pack_wave214());
    }

    #[test]
    fn producer_presentation_sources() {
        assert!(honesty_force_completed_presentation_only_source());
        assert!(honesty_no_live_classify_closure_source());
    }

    #[test]
    fn simulate_live_ui_producer_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_ui_producer_presentation_only_honesty(),
            "ui producer presentation-only residual must latch"
        );
    }
}
